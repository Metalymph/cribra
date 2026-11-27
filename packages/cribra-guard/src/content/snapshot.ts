/**
 * Bounded, transient snapshot store.
 *
 * The store retains exact logical source only in memory and only while a
 * snapshot is valid. Invalidation is sticky: once a snapshot is invalidated it
 * can never become valid again, even if the editor later returns to the same
 * text. Identifiers are never reused. Source is cleared on invalidation,
 * release, disposal, or eviction.
 *
 * Both the snapshot count and the aggregate retained source are bounded. The
 * per-source limit matches the scanner protocol's 1,000,000 UTF-16 code-unit
 * limit; the content layer keeps a local constant because it must not import the
 * WASM-coupled protocol module.
 *
 * This module does not observe the DOM. The interceptor feeds observable
 * changes into the store and asks it to verify the current editor text.
 */

import type {
  AdapterIdentity,
  EditorKind,
  EditorRef,
  ReplacementResult,
} from "./contract.js";
import { newSnapshotId } from "./tokens.js";

export const MAX_SNAPSHOTS = 64;

/**
 * Per-snapshot source limit. Must stay aligned with
 * `src/protocol.ts` `MAX_SOURCE_LENGTH`.
 */
export const MAX_SNAPSHOT_SOURCE_LENGTH = 1_000_000;

/**
 * Aggregate source retained across valid snapshots. This is intentionally a
 * small multiple of the per-source limit, not an unbounded accumulation across
 * the maximum snapshot count.
 */
export const MAX_RETAINED_SOURCE_CHARS = 4_000_000;

export type SnapshotStatus = "valid" | "invalidated" | "released" | "unknown";

export type SnapshotInvalidationReason =
  | "source_changed"
  | "editor_detached"
  | "editor_replaced"
  | "form_detached"
  | "form_replaced"
  | "document_replaced"
  | "replaced"
  | "replacement_failed"
  | "superseded"
  | "released"
  | "adapter_invalidated"
  | "suspended"
  | "disposed";

/** Source-free metadata safe for diagnostics and qualification hooks. */
export interface SnapshotMetadata {
  readonly snapshotId: string;
  readonly identity: AdapterIdentity;
  readonly editorKind: EditorKind;
  readonly formToken: string;
  readonly editorToken: string;
  readonly documentToken: string;
  readonly status: SnapshotStatus;
  readonly invalidationReason?: SnapshotInvalidationReason;
  readonly createdAt: number;
}

interface SnapshotRecord {
  readonly snapshotId: string;
  readonly identity: AdapterIdentity;
  readonly editorKind: EditorKind;
  readonly formToken: string;
  readonly editorToken: string;
  readonly documentToken: string;
  readonly editor: EditorRef;
  readonly createdAt: number;
  status: SnapshotStatus;
  invalidationReason?: SnapshotInvalidationReason;
  source: string;
}

export interface SnapshotCounts {
  readonly total: number;
  readonly valid: number;
  readonly invalidated: number;
  readonly released: number;
  /** UTF-16 code units currently retained across valid snapshots. */
  readonly retainedSourceChars: number;
}

function toMetadata(record: SnapshotRecord): SnapshotMetadata {
  return {
    snapshotId: record.snapshotId,
    identity: record.identity,
    editorKind: record.editorKind,
    formToken: record.formToken,
    editorToken: record.editorToken,
    documentToken: record.documentToken,
    status: record.status,
    ...(record.invalidationReason === undefined
      ? {}
      : { invalidationReason: record.invalidationReason }),
    createdAt: record.createdAt,
  };
}

export class SnapshotStore {
  private readonly records = new Map<string, SnapshotRecord>();
  private readonly order: string[] = [];
  private retainedSourceChars = 0;
  private disposed = false;

  get size(): number {
    return this.records.size;
  }

  /**
   * Creates a valid snapshot. Returns undefined when the store is disposed or
   * cannot allocate. The caller must never expose `source`.
   */
  create(
    identity: AdapterIdentity,
    editor: EditorRef,
    formToken: string,
    documentToken: string,
    source: string,
  ): SnapshotMetadata | undefined {
    if (this.disposed) return undefined;
    if (source.length > MAX_SNAPSHOT_SOURCE_LENGTH) return undefined;
    this.evictIfNeeded(source.length);
    if (
      this.retainedSourceChars + source.length >
      MAX_RETAINED_SOURCE_CHARS
    ) {
      return undefined;
    }
    const snapshotId = newSnapshotId();
    const record: SnapshotRecord = {
      snapshotId,
      identity,
      editorKind: editor.kind,
      formToken,
      editorToken: editor.token,
      documentToken,
      editor,
      createdAt: Date.now(),
      status: "valid",
      source,
    };
    this.records.set(snapshotId, record);
    this.order.push(snapshotId);
    this.retainedSourceChars += source.length;
    return toMetadata(record);
  }

  metadata(snapshotId: string): SnapshotMetadata | undefined {
    const record = this.records.get(snapshotId);
    return record === undefined ? undefined : toMetadata(record);
  }

  status(snapshotId: string): SnapshotStatus {
    return this.records.get(snapshotId)?.status ?? "unknown";
  }

  /**
   * Flushes nothing itself; the interceptor must process pending mutation
   * records before calling this. Returns false and permanently invalidates the
   * snapshot when the document, editor, form, or exact source no longer match.
   */
  verify(
    snapshotId: string,
    documentToken: string,
    read: (editor: EditorRef) => string,
    isCurrent: (editor: EditorRef) => boolean,
  ): boolean {
    const record = this.records.get(snapshotId);
    if (record === undefined || record.status !== "valid") return false;
    if (record.documentToken !== documentToken) {
      this.invalidate(snapshotId, "document_replaced");
      return false;
    }
    if (!isCurrent(record.editor)) {
      this.invalidate(
        snapshotId,
        record.editor.form.isConnected ? "editor_detached" : "form_detached",
      );
      return false;
    }
    if (read(record.editor) !== record.source) {
      this.invalidate(snapshotId, "source_changed");
      return false;
    }
    return true;
  }

  /**
   * Qualification-only exact-equality oracle.
   *
   * It never returns source. A mismatch invalidates the snapshot, mirroring the
   * sticky observable-change rule.
   */
  sourceEquals(
    snapshotId: string,
    documentToken: string,
    candidate: string,
    read: (editor: EditorRef) => string,
    isCurrent: (editor: EditorRef) => boolean,
  ): boolean {
    if (!this.verify(snapshotId, documentToken, read, isCurrent)) return false;
    const record = this.records.get(snapshotId);
    if (record === undefined) return false;
    if (record.source !== candidate) {
      this.invalidate(snapshotId, "source_changed");
      return false;
    }
    return true;
  }

  /**
   * Runs a controlled replacement and always invalidates the original snapshot.
   * A successful replacement must never preserve the previous authorization
   * identity. The future controller establishes a new snapshot identity and
   * performs transformation revalidation.
   */
  replace(
    snapshotId: string,
    text: string,
    replace: (editor: EditorRef, text: string) => ReplacementResult,
  ): ReplacementResult {
    const record = this.records.get(snapshotId);
    if (record === undefined || record.status !== "valid") {
      return { status: "failed", reason: "snapshot_unavailable" };
    }
    let result: ReplacementResult;
    try {
      result = replace(record.editor, text);
    } catch {
      result = { status: "failed", reason: "write_failed" };
    }
    this.invalidate(
      snapshotId,
      result.status === "replaced" ? "replaced" : "replacement_failed",
    );
    return result;
  }

  focus(
    snapshotId: string,
    documentToken: string,
    focus: (editor: EditorRef) => void,
    isCurrent: (editor: EditorRef) => boolean,
  ): boolean {
    const record = this.records.get(snapshotId);
    if (record === undefined || record.status !== "valid") return false;
    if (record.documentToken !== documentToken || !isCurrent(record.editor)) {
      this.invalidate(snapshotId, "editor_detached");
      return false;
    }
    focus(record.editor);
    return true;
  }

  invalidate(snapshotId: string, reason: SnapshotInvalidationReason): boolean {
    const record = this.records.get(snapshotId);
    if (record === undefined || record.status !== "valid") return false;
    record.status = "invalidated";
    record.invalidationReason = reason;
    this.dropSource(record);
    return true;
  }

  release(snapshotId: string): boolean {
    const record = this.records.get(snapshotId);
    if (record === undefined || record.status !== "valid") return false;
    record.status = "released";
    record.invalidationReason = "released";
    this.dropSource(record);
    return true;
  }

  invalidateEditor(
    editorToken: string,
    reason: SnapshotInvalidationReason,
  ): number {
    let count = 0;
    for (const record of this.records.values()) {
      if (record.editorToken === editorToken && record.status === "valid") {
        if (this.invalidate(record.snapshotId, reason)) count += 1;
      }
    }
    return count;
  }

  invalidateForm(formToken: string, reason: SnapshotInvalidationReason): number {
    let count = 0;
    for (const record of this.records.values()) {
      if (record.formToken === formToken && record.status === "valid") {
        if (this.invalidate(record.snapshotId, reason)) count += 1;
      }
    }
    return count;
  }

  invalidateAll(reason: SnapshotInvalidationReason): number {
    let count = 0;
    for (const record of this.records.values()) {
      if (record.status === "valid") {
        if (this.invalidate(record.snapshotId, reason)) count += 1;
      }
    }
    return count;
  }

  pendingForEditor(editorToken: string): SnapshotMetadata | undefined {
    for (const snapshotId of this.order) {
      const record = this.records.get(snapshotId);
      if (
        record !== undefined &&
        record.status === "valid" &&
        record.editorToken === editorToken
      ) {
        return toMetadata(record);
      }
    }
    return undefined;
  }

  pending(): readonly string[] {
    return this.order.filter(
      (snapshotId) => this.records.get(snapshotId)?.status === "valid",
    );
  }

  counts(): SnapshotCounts {
    let valid = 0;
    let invalidated = 0;
    let released = 0;
    for (const record of this.records.values()) {
      if (record.status === "valid") valid += 1;
      else if (record.status === "invalidated") invalidated += 1;
      else if (record.status === "released") released += 1;
    }
    return {
      total: this.records.size,
      valid,
      invalidated,
      released,
      retainedSourceChars: this.retainedSourceChars,
    };
  }

  dispose(): void {
    if (this.disposed) return;
    this.invalidateAll("disposed");
    this.records.clear();
    this.order.length = 0;
    this.retainedSourceChars = 0;
    this.disposed = true;
  }

  private evictIfNeeded(incomingSourceLength: number): void {
    while (
      this.records.size >= MAX_SNAPSHOTS ||
      this.retainedSourceChars + incomingSourceLength >
        MAX_RETAINED_SOURCE_CHARS
    ) {
      if (this.records.size === 0) return;
      const removableIndex = this.order.findIndex(
        (snapshotId) => this.records.get(snapshotId)?.status !== "valid",
      );
      const index = removableIndex === -1 ? 0 : removableIndex;
      const [snapshotId] = this.order.splice(index, 1);
      if (snapshotId !== undefined) this.deleteRecord(snapshotId);
    }
  }

  private dropSource(record: SnapshotRecord): void {
    if (record.source.length === 0) return;
    this.retainedSourceChars -= record.source.length;
    if (this.retainedSourceChars < 0) this.retainedSourceChars = 0;
    record.source = "";
  }

  private deleteRecord(snapshotId: string): void {
    const record = this.records.get(snapshotId);
    if (record === undefined) return;
    this.retainedSourceChars -= record.source.length;
    if (this.retainedSourceChars < 0) this.retainedSourceChars = 0;
    this.records.delete(snapshotId);
  }
}
