/**
 * Synchronous submission interception.
 *
 * A document capture-phase `submit` listener classifies the form, cancels the
 * native default for supported or known-invalidated paths, and captures an
 * exact source snapshot before any asynchronous work. Intercepted operations
 * remain blocked: this milestone never resumes, resubmits, or dispatches a
 * submission.
 *
 * `preventDefault` plus `stopImmediatePropagation` does not stop direct
 * fetch/XHR, a prior capture handler on `window`, non-cancellable events, or
 * `form.submit()`. Those limitations are reported, not hidden.
 */

import type {
  AdapterIdentity,
  CaptureResult,
  CapturedResult,
  CoverageFailureReason,
  CoverageLossReason,
  CoverageLossReport,
  DegradedCaptureResult,
  DuplicateResult,
  InvalidatedCaptureResult,
  NotCancellableResult,
  ReplacementResult,
  SupportedCoverage,
  UnsupportedIgnoredResult,
} from "./contract.js";
import {
  focusEditor,
  formFromEventTarget,
  isEditorCurrent,
  readEditor,
  replaceEditor,
  submitterKindOf,
} from "./editors.js";
import { AdapterRegistry, REGISTRY_IDENTITY } from "./registry.js";
import {
  MAX_SNAPSHOT_SOURCE_LENGTH,
  SnapshotStore,
  type SnapshotMetadata,
  type SnapshotStatus,
} from "./snapshot.js";
import { objectToken } from "./tokens.js";

export interface InterceptorOptions {
  readonly registry?: AdapterRegistry;
  readonly store?: SnapshotStore;
  readonly document?: Document;
  readonly now?: () => number;
  readonly maxHistory?: number;
  readonly onCapture?: (result: CaptureResult) => void;
  readonly observerFactory?: (
    callback: MutationCallback,
  ) => MutationObserver;
}

const DEFAULT_MAX_HISTORY = 64;

export class SubmissionInterceptor {
  private readonly registry: AdapterRegistry;
  private readonly store: SnapshotStore;
  private readonly targetDocument: Document;
  private readonly now: () => number;
  private readonly maxHistory: number;
  private readonly onCapture:
    | ((result: CaptureResult) => void)
    | undefined;
  private readonly observerFactory:
    | ((callback: MutationCallback) => MutationObserver)
    | undefined;
  private readonly documentToken: string;
  private readonly captureHistory: CaptureResult[] = [];
  private observer: MutationObserver | undefined;
  private installed = false;
  private disposed = false;
  private suspensions = 0;
  private restorations = 0;

  private readonly onSubmit = (event: Event): void => {
    try {
      this.handleSubmit(event as SubmitEvent);
    } catch {
      const form = formFromEventTarget((event as SubmitEvent).target);
      if (form !== null) this.handleDetectionFailure(event as SubmitEvent, form);
    }
  };

  private readonly onInput = (event: Event): void => {
    try {
      this.handleObservableChange(event);
    } catch {
      this.store.invalidateAll("adapter_invalidated");
    }
  };

  private readonly onPageHide = (event: Event): void => {
    this.handlePageHide(event as PageTransitionEvent);
  };

  private readonly onPageShow = (event: Event): void => {
    this.handlePageShow(event as PageTransitionEvent);
  };

  constructor(options: InterceptorOptions = {}) {
    const documentValue =
      options.document ??
      (typeof document === "undefined" ? undefined : document);
    if (documentValue === undefined) {
      throw new Error("SubmissionInterceptor requires a document");
    }
    this.targetDocument = documentValue;
    this.registry = options.registry ?? new AdapterRegistry();
    this.store = options.store ?? new SnapshotStore();
    this.now = options.now ?? (() => Date.now());
    this.maxHistory = options.maxHistory ?? DEFAULT_MAX_HISTORY;
    this.onCapture = options.onCapture;
    this.observerFactory = options.observerFactory;
    this.documentToken = objectToken(this.targetDocument, "doc");
  }

  install(): void {
    if (this.installed || this.disposed) return;
    this.targetDocument.addEventListener("submit", this.onSubmit, true);
    this.targetDocument.addEventListener("input", this.onInput, true);
    this.targetDocument.addEventListener("change", this.onInput, true);
    this.targetDocument.defaultView?.addEventListener(
      "pagehide",
      this.onPageHide,
    );
    this.targetDocument.defaultView?.addEventListener(
      "pageshow",
      this.onPageShow,
    );
    const factory =
      this.observerFactory ??
      ((callback: MutationCallback) => new MutationObserver(callback));
    this.observer = factory((records) => {
      try {
        this.processMutations(records);
      } catch {
        // A mutation-processing failure must not leave a stale snapshot valid.
        this.store.invalidateAll("adapter_invalidated");
      }
    });
    this.observer.observe(this.targetDocument, {
      subtree: true,
      childList: true,
      characterData: true,
      attributes: true,
      attributeFilter: ["contenteditable", "readonly", "disabled", "type"],
    });
    this.installed = true;
  }

  isInstalled(): boolean {
    return this.installed && !this.disposed;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.installed = false;
    this.observer?.disconnect();
    this.observer = undefined;
    this.targetDocument.removeEventListener("submit", this.onSubmit, true);
    this.targetDocument.removeEventListener("input", this.onInput, true);
    this.targetDocument.removeEventListener("change", this.onInput, true);
    this.targetDocument.defaultView?.removeEventListener(
      "pagehide",
      this.onPageHide,
    );
    this.targetDocument.defaultView?.removeEventListener(
      "pageshow",
      this.onPageShow,
    );
    this.store.dispose();
    this.registry.dispose();
    this.captureHistory.length = 0;
  }

  /** Public for unit tests; the installed listener calls this synchronously. */
  handleSubmit(event: SubmitEvent): void {
    if (this.disposed) return;
    const form = formFromEventTarget(event.target);
    if (form === null) return;

    try {
      this.flushMutations();
    } catch {
      this.handleDetectionFailure(event, form);
      return;
    }
    let coverage: ReturnType<AdapterRegistry["detect"]>;
    try {
      coverage = this.registry.detect(form);
    } catch {
      // A recognized adapter must never fail open. Detection errors are
      // treated as an invalidated protected path and cancelled where possible.
      this.handleDetectionFailure(event, form);
      return;
    }

    switch (coverage.status) {
      case "supported":
        this.handleSupported(event, coverage);
        return;
      case "unsupported":
        if (this.registry.isKnown(form)) {
          this.handleFailure(
            event,
            coverage.identity,
            objectToken(form, "form"),
            "adapter_invalidated",
            "invalidated",
            coverage.reason,
          );
        } else {
          this.record({
            kind: "unsupported_ignored",
            identity: coverage.identity,
            reason: coverage.reason,
            blocked: false,
            cancelled: false,
            at: this.now(),
          });
        }
        return;
      case "invalidated":
        this.handleFailure(
          event,
          coverage.identity,
          objectToken(form, "form"),
          coverage.reason,
          "invalidated",
          coverage.cause,
        );
        return;
      case "degraded":
        this.handleFailure(
          event,
          coverage.identity,
          objectToken(form, "form"),
          coverage.reason,
          "degraded",
          undefined,
        );
        return;
    }
  }

  /**
   * Persisted pagehide means the document may be restored from the
   * Back/Forward Cache. Keep the listener installed, but invalidate every
   * snapshot and clear retained source while frozen. A non-persisted pagehide
   * is unload and disposes the interceptor.
   */
  handlePageHide(event: PageTransitionEvent): void {
    if (this.disposed) return;
    if (event.persisted === true) {
      this.suspensions += 1;
      this.suspend();
      return;
    }
    this.dispose();
  }

  /** A persisted pageshow keeps automatic interception active after restore. */
  handlePageShow(event: PageTransitionEvent): void {
    if (this.disposed || event.persisted !== true) return;
    this.restorations += 1;
    if (!this.installed) this.install();
  }

  verifySnapshot(snapshotId: string): boolean {
    if (this.disposed) return false;
    this.flushMutations();
    const metadata = this.store.metadata(snapshotId);
    if (metadata === undefined) return false;
    if (metadata.documentToken !== this.documentToken) {
      this.store.invalidate(snapshotId, "document_replaced");
      return false;
    }
    return this.store.verify(
      snapshotId,
      this.documentToken,
      readEditor,
      isEditorCurrent,
    );
  }

  sourceEquals(snapshotId: string, candidate: string): boolean {
    if (this.disposed) return false;
    this.flushMutations();
    return this.store.sourceEquals(
      snapshotId,
      this.documentToken,
      candidate,
      readEditor,
      isEditorCurrent,
    );
  }

  replaceSnapshot(snapshotId: string, text: string): ReplacementResult {
    if (this.disposed) {
      return { status: "failed", reason: "snapshot_unavailable" };
    }
    this.flushMutations();
    return this.store.replace(snapshotId, text, replaceEditor);
  }

  focusSnapshot(snapshotId: string): boolean {
    if (this.disposed) return false;
    this.flushMutations();
    return this.store.focus(
      snapshotId,
      this.documentToken,
      focusEditor,
      isEditorCurrent,
    );
  }

  releaseSnapshot(snapshotId: string): boolean {
    return this.store.release(snapshotId);
  }

  metadata(snapshotId: string): SnapshotMetadata | undefined {
    return this.store.metadata(snapshotId);
  }

  status(snapshotId: string): SnapshotStatus {
    return this.store.status(snapshotId);
  }

  pending(): readonly string[] {
    return this.store.pending();
  }

  history(): readonly CaptureResult[] {
    return [...this.captureHistory];
  }

  losses(): readonly CoverageLossReport[] {
    return this.registry.losses();
  }

  counts(): {
    readonly captures: number;
    readonly duplicates: number;
    readonly invalidated: number;
    readonly degraded: number;
    readonly notCancellable: number;
    readonly unsupportedIgnored: number;
    readonly snapshots: number;
    readonly pending: number;
    readonly losses: number;
    readonly retainedSourceChars: number;
    readonly suspensions: number;
    readonly restorations: number;
  } {
    const count = (kind: CaptureResult["kind"]): number =>
      this.captureHistory.filter((entry) => entry.kind === kind).length;
    return {
      captures: count("captured"),
      duplicates: count("duplicate"),
      invalidated: count("invalidated"),
      degraded: count("degraded"),
      notCancellable: count("not_cancellable"),
      unsupportedIgnored: count("unsupported_ignored"),
      snapshots: this.store.size,
      pending: this.store.pending().length,
      losses: this.registry.losses().length,
      retainedSourceChars: this.store.counts().retainedSourceChars,
      suspensions: this.suspensions,
      restorations: this.restorations,
    };
  }

  private suspend(): void {
    this.store.invalidateAll("suspended");
  }

  private handleDetectionFailure(
    event: SubmitEvent,
    form: HTMLFormElement,
  ): void {
    const identity = this.registry.identityFor(form) ?? REGISTRY_IDENTITY;
    const formToken = objectToken(form, "form");
    if (event.cancelable !== true) {
      this.record({
        kind: "not_cancellable",
        identity,
        formToken,
        reason: "adapter_invalidated",
        blocked: false,
        cancelled: false,
        at: this.now(),
      });
      this.recordLoss(identity, formToken, "not_cancellable");
      return;
    }
    this.cancel(event);
    this.record({
      kind: "invalidated",
      identity,
      formToken,
      reason: "adapter_invalidated",
      cause: "adapter_invalidated",
      blocked: true,
      cancelled: true,
      at: this.now(),
    });
    this.recordLoss(identity, formToken, "adapter_invalidated");
  }

  private handleSupported(
    event: SubmitEvent,
    coverage: SupportedCoverage,
  ): void {
    const submitter = event.submitter ?? null;
    let supportedSubmitter: boolean;
    try {
      supportedSubmitter = coverage.adapter.supportsSubmitter(
        coverage,
        submitter,
      );
    } catch {
      this.handleFailure(
        event,
        coverage.identity,
        coverage.formToken,
        "adapter_invalidated",
        "invalidated",
        "adapter_invalidated",
      );
      return;
    }
    if (!supportedSubmitter) {
      this.handleFailure(
        event,
        coverage.identity,
        coverage.formToken,
        "unsupported_submitter",
        "invalidated",
        "unsupported_submitter",
      );
      return;
    }

    if (event.cancelable !== true) {
      const result: NotCancellableResult = {
        kind: "not_cancellable",
        identity: coverage.identity,
        formToken: coverage.formToken,
        reason: "not_cancellable",
        blocked: false,
        cancelled: false,
        at: this.now(),
      };
      this.record(result);
      this.recordLoss(coverage.identity, coverage.formToken, "not_cancellable");
      return;
    }

    const pending = this.store.pendingForEditor(coverage.editor.token);
    if (pending !== undefined) {
      this.cancel(event);
      const result: DuplicateResult = {
        kind: "duplicate",
        identity: coverage.identity,
        snapshotId: pending.snapshotId,
        formToken: coverage.formToken,
        blocked: true,
        cancelled: true,
        at: this.now(),
      };
      this.record(result);
      return;
    }

    // Cancel before the synchronous read so a native submission cannot race.
    this.cancel(event);

    let source: string;
    try {
      source = coverage.adapter.read(coverage);
    } catch {
      this.handleFailure(
        event,
        coverage.identity,
        coverage.formToken,
        "adapter_invalidated",
        "invalidated",
        "unsupported_editor",
      );
      return;
    }

    if (source.length > MAX_SNAPSHOT_SOURCE_LENGTH) {
      this.handleFailure(
        event,
        coverage.identity,
        coverage.formToken,
        "oversized_source",
        "invalidated",
        "oversized_source",
      );
      return;
    }

    let metadata: SnapshotMetadata | undefined;
    try {
      metadata = this.store.create(
        coverage.identity,
        coverage.editor,
        coverage.formToken,
        this.documentToken,
        source,
      );
    } catch {
      this.handleFailure(
        event,
        coverage.identity,
        coverage.formToken,
        "adapter_invalidated",
        "invalidated",
        "adapter_invalidated",
      );
      return;
    }
    if (metadata === undefined) {
      // The store rejects source beyond its per-snapshot or aggregate budget.
      // The event is already cancelled and no snapshot is retained.
      const result: InvalidatedCaptureResult = {
        kind: "invalidated",
        identity: coverage.identity,
        formToken: coverage.formToken,
        reason: "oversized_source",
        cause: "oversized_source",
        blocked: true,
        cancelled: true,
        at: this.now(),
      };
      this.record(result);
      return;
    }

    const result: CapturedResult = {
      kind: "captured",
      identity: coverage.identity,
      snapshotId: metadata.snapshotId,
      formToken: coverage.formToken,
      submitterToken:
        submitter === null ? null : objectToken(submitter, "submitter"),
      submitterKind: submitterKindOf(submitter),
      editorKind: coverage.editor.kind,
      blocked: true,
      cancelled: true,
      at: this.now(),
    };
    this.record(result);
  }

  private handleFailure(
    event: SubmitEvent,
    identity: AdapterIdentity,
    formToken: string,
    reason: CoverageFailureReason,
    kind: "invalidated" | "degraded",
    cause: CoverageFailureReason | undefined,
  ): void {
    if (event.cancelable !== true) {
      const result: NotCancellableResult = {
        kind: "not_cancellable",
        identity,
        formToken,
        reason,
        blocked: false,
        cancelled: false,
        at: this.now(),
      };
      this.record(result);
      this.recordLoss(identity, formToken, "not_cancellable");
      return;
    }

    this.cancel(event);
    if (kind === "degraded") {
      const result: DegradedCaptureResult = {
        kind: "degraded",
        identity,
        formToken,
        reason,
        blocked: true,
        cancelled: true,
        at: this.now(),
      };
      this.record(result);
      return;
    }

    const result: InvalidatedCaptureResult = {
      kind: "invalidated",
      identity,
      formToken,
      reason,
      ...(cause === undefined ? {} : { cause }),
      blocked: true,
      cancelled: true,
      at: this.now(),
    };
    this.record(result);
    this.recordLoss(identity, formToken, "adapter_invalidated");
  }

  private cancel(event: SubmitEvent): void {
    if (typeof event.preventDefault === "function") event.preventDefault();
    if (typeof event.stopImmediatePropagation === "function") {
      event.stopImmediatePropagation();
    }
  }

  private handleObservableChange(event: Event): void {
    if (this.disposed) return;
    const target = event.target;
    if (target === null || typeof (target as Node).nodeType !== "number") {
      return;
    }
    for (const record of this.registry.protectedRecords()) {
      const editor = record.coverage.editor.element;
      if (editor === target || editor.contains(target as Node)) {
        this.store.invalidateEditor(
          record.coverage.editor.token,
          "source_changed",
        );
      }
    }
  }

  private flushMutations(): void {
    if (this.observer === undefined) return;
    this.processMutations(this.observer.takeRecords());
  }

  private processMutations(records: readonly MutationRecord[]): void {
    if (this.disposed) return;
    for (const protectedRecord of this.registry.protectedRecords()) {
      const { coverage } = protectedRecord;
      const form = coverage.editor.form;
      const editor = coverage.editor.element;

      if (form.isConnected !== true) {
        this.store.invalidateForm(coverage.formToken, "form_detached");
        this.recordLoss(coverage.identity, coverage.formToken, "form_replaced");
        continue;
      }
      if (editor.isConnected !== true || form.contains(editor) !== true) {
        this.store.invalidateEditor(
          coverage.editor.token,
          "editor_detached",
        );
        this.recordLoss(
          coverage.identity,
          coverage.formToken,
          "editor_replaced",
        );
        continue;
      }

      for (const mutation of records) {
        if (mutation.type === "attributes" && mutation.target === editor) {
          this.store.invalidateEditor(
            coverage.editor.token,
            "adapter_invalidated",
          );
          continue;
        }
        const target = mutation.target;
        const inEditor =
          target === editor ||
          (typeof (target as Node).nodeType === "number" &&
            editor.contains(target as Node));
        if (!inEditor) continue;
        if (mutation.type === "characterData" || mutation.type === "childList") {
          this.store.invalidateEditor(
            coverage.editor.token,
            "source_changed",
          );
        }
      }
    }
  }

  private recordLoss(
    identity: AdapterIdentity,
    formToken: string,
    reason: CoverageLossReason,
  ): void {
    this.registry.recordLoss({
      kind: "coverage_lost",
      identity,
      formToken,
      reason,
      at: this.now(),
    });
  }

  private record(result: CaptureResult): void {
    this.captureHistory.push(result);
    while (this.captureHistory.length > this.maxHistory) {
      this.captureHistory.shift();
    }
    try {
      this.onCapture?.(result);
    } catch {
      // A future controller listener must never break interception.
    }
  }
}
