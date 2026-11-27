/**
 * Deterministic content-layer unit tests.
 *
 * These tests use a tiny in-memory DOM subset. Real browser events, real form
 * submission, local endpoints, and isolated-world access are exercised by
 * `npm run qualify:content`.
 */

import assert from "node:assert/strict";
import { describe, test } from "node:test";

import type {
  AdapterCoverage,
  AdapterIdentity,
  EditorRef,
  PageAdapter,
  ReplacementResult,
  SupportedCoverage,
} from "./contract.js";
import { ContentEditableAdapter } from "./contenteditable-adapter.js";
import {
  isEditorCurrent,
  readEditor,
  replaceEditor,
} from "./editors.js";
import { SubmissionInterceptor } from "./interceptor.js";
import {
  EDITOR_MARKER,
  EDITOR_VALUE,
  NativeFormAdapter,
  NATIVE_FORM_MARKER,
  NATIVE_FORM_VALUE,
} from "./native-form-adapter.js";
import { installQualificationHook } from "./qualification.js";
import { AdapterRegistry } from "./registry.js";
import {
  MAX_RETAINED_SOURCE_CHARS,
  MAX_SNAPSHOT_SOURCE_LENGTH,
  MAX_SNAPSHOTS,
  SnapshotStore,
  type SnapshotMetadata,
} from "./snapshot.js";
import {
  newSnapshotId,
  objectToken,
  tokenCountersForTest,
} from "./tokens.js";

const IDENTITY: AdapterIdentity = { id: "test.adapter", version: "1.0.0" };

class FakeWindow {
  readonly listeners = new Map<string, Set<EventListener>>();

  addEventListener(type: string, listener: EventListener): void {
    const set = this.listeners.get(type) ?? new Set<EventListener>();
    set.add(listener);
    this.listeners.set(type, set);
  }

  removeEventListener(type: string, listener: EventListener): void {
    this.listeners.get(type)?.delete(listener);
  }
}

class FakeElement {
  readonly tagName: string;
  readonly attributes = new Map<string, string>();
  readonly children: FakeElement[] = [];
  readonly dispatchedEvents: Event[] = [];
  readonly listeners = new Map<string, Set<EventListener>>();
  parentNode: FakeElement | null = null;
  type = "";
  form: FakeElement | null = null;
  readOnly = false;
  disabled = false;
  private currentValue = "";
  private currentText = "";

  constructor(tagName: string) {
    this.tagName = tagName.toUpperCase();
  }

  get isConnected(): boolean {
    let current: FakeElement | null = this;
    while (current !== null) {
      if (current instanceof FakeDocument) return true;
      current = current.parentNode;
    }
    return false;
  }

  get ownerDocument(): FakeDocument | null {
    let current: FakeElement | null = this;
    while (current !== null) {
      if (current instanceof FakeDocument) return current;
      current = current.parentNode;
    }
    return null;
  }

  get value(): string {
    return this.currentValue;
  }

  set value(next: string) {
    // Browsers strip CR/LF from single-line text inputs. Textareas preserve
    // them. The read-back comparison must detect the difference.
    this.currentValue =
      this.tagName === "INPUT" ? next.replace(/[\r\n]/g, "") : next;
  }

  get textContent(): string {
    return this.currentText;
  }

  set textContent(next: string) {
    this.currentText = next;
  }

  get isContentEditable(): boolean {
    const own = this.getAttribute("contenteditable");
    if (own === "false") return false;
    if (own !== null) return true;
    let current = this.parentNode;
    while (current !== null) {
      const inherited = current.getAttribute("contenteditable");
      if (inherited === "false") return false;
      if (inherited !== null) return true;
      current = current.parentNode;
    }
    return false;
  }

  getAttribute(name: string): string | null {
    return this.attributes.get(name) ?? null;
  }

  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }

  removeAttribute(name: string): void {
    this.attributes.delete(name);
  }

  hasAttribute(name: string): boolean {
    return this.attributes.has(name);
  }

  appendChild(child: FakeElement): FakeElement {
    child.parentNode = this;
    this.children.push(child);
    return child;
  }

  removeChild(child: FakeElement): FakeElement {
    const index = this.children.indexOf(child);
    if (index !== -1) this.children.splice(index, 1);
    child.parentNode = null;
    return child;
  }

  contains(candidate: unknown): boolean {
    if (!(candidate instanceof FakeElement)) return false;
    let current: FakeElement | null = candidate;
    while (current !== null) {
      if (current === this) return true;
      current = current.parentNode;
    }
    return false;
  }

  replaceWith(replacement: FakeElement): void {
    const parent = this.parentNode;
    if (parent === null) return;
    const index = parent.children.indexOf(this);
    if (index === -1) return;
    parent.children[index] = replacement;
    replacement.parentNode = parent;
    this.parentNode = null;
  }

  remove(): void {
    const parent = this.parentNode;
    if (parent !== null) parent.removeChild(this);
  }

  querySelectorAll<T extends Element = Element>(selector: string): T[] {
    const matches: FakeElement[] = [];
    for (const descendant of this.descendants()) {
      if (descendant.matchesSelector(selector)) matches.push(descendant);
    }
    return matches as unknown as T[];
  }

  matchesSelector(selector: string): boolean {
    if (selector === "form") return this.tagName === "FORM";
    const match = /^\[([^\]=]+)(?:="([^"]*)")?\]$/.exec(selector);
    if (match === null) return false;
    const name = match[1];
    if (name === undefined) return false;
    const actual = this.getAttribute(name);
    if (actual === null) return false;
    return match[2] === undefined || actual === match[2];
  }

  closest(selector: string): FakeElement | null {
    let current: FakeElement | null = this;
    while (current !== null) {
      if (current.matchesSelector(selector)) return current;
      current = current.parentNode;
    }
    return null;
  }

  dispatchEvent(event: Event): boolean {
    this.dispatchedEvents.push(event);
    return true;
  }

  addEventListener(type: string, listener: EventListener): void {
    const set = this.listeners.get(type) ?? new Set<EventListener>();
    set.add(listener);
    this.listeners.set(type, set);
  }

  removeEventListener(type: string, listener: EventListener): void {
    this.listeners.get(type)?.delete(listener);
  }

  focus(): void {
    const document = this.ownerDocument;
    if (document !== null) document.activeElement = this;
  }

  private *descendants(): Iterable<FakeElement> {
    for (const child of this.children) {
      yield child;
      yield* child.descendants();
    }
  }
}

class FakeDocument extends FakeElement {
  readonly defaultView = new FakeWindow();
  activeElement: FakeElement | null = null;

  constructor() {
    super("#document");
  }

  createElement(tagName: string): FakeElement {
    return new FakeElement(tagName);
  }
}

class FakeMutationObserver {
  constructor(readonly callback: MutationCallback) {}

  observe(): void {}

  disconnect(): void {}

  takeRecords(): MutationRecord[] {
    return [];
  }
}

interface FakeSubmitEvent {
  target: FakeElement;
  cancelable: boolean;
  submitter: FakeElement | null;
  defaultPrevented: boolean;
  propagationStopped: boolean;
  preventDefault(): void;
  stopImmediatePropagation(): void;
}

function submitEvent(
  form: FakeElement,
  options: { cancelable?: boolean; submitter?: FakeElement | null } = {},
): FakeSubmitEvent {
  return {
    target: form,
    cancelable: options.cancelable ?? true,
    submitter: options.submitter ?? null,
    defaultPrevented: false,
    propagationStopped: false,
    preventDefault() {
      this.defaultPrevented = true;
    },
    stopImmediatePropagation() {
      this.propagationStopped = true;
    },
  };
}

interface NativeSetup {
  readonly document: FakeDocument;
  readonly form: FakeElement;
  readonly editor: FakeElement;
  readonly submitter: FakeElement;
}

function setupNative(
  options: {
    readonly marker?: string | null;
    readonly editorTag?: "textarea" | "input";
    readonly value?: string;
    readonly readOnly?: boolean;
    readonly disabled?: boolean;
    readonly editorType?: string;
  } = {},
): NativeSetup {
  const document = new FakeDocument();
  const form = document.createElement("form");
  if (options.marker !== null) {
    form.setAttribute(
      NATIVE_FORM_MARKER,
      options.marker ?? NATIVE_FORM_VALUE,
    );
  }
  document.appendChild(form);

  const editor = document.createElement(options.editorTag ?? "textarea");
  if (editor.tagName === "INPUT") {
    editor.type = options.editorType ?? "text";
  }
  editor.setAttribute(EDITOR_MARKER, EDITOR_VALUE);
  editor.value = options.value ?? "fixture source";
  editor.readOnly = options.readOnly ?? false;
  editor.disabled = options.disabled ?? false;
  form.appendChild(editor);

  const submitter = document.createElement("button");
  submitter.type = "submit";
  submitter.form = form;
  form.appendChild(submitter);

  return { document, form, editor, submitter };
}

interface ContentEditableSetup {
  readonly document: FakeDocument;
  readonly form: FakeElement;
  readonly editor: FakeElement;
  readonly submitter: FakeElement;
}

function setupContentEditable(
  options: {
    readonly contenteditable?: string;
    readonly value?: string;
  } = {},
): ContentEditableSetup {
  const document = new FakeDocument();
  const form = document.createElement("form");
  form.setAttribute("data-cribra-adapter", "contenteditable");
  document.appendChild(form);

  const editor = document.createElement("div");
  editor.setAttribute(
    "contenteditable",
    options.contenteditable ?? "true",
  );
  editor.setAttribute(EDITOR_MARKER, EDITOR_VALUE);
  editor.textContent = options.value ?? "fixture source";
  form.appendChild(editor);

  const submitter = document.createElement("button");
  submitter.type = "submit";
  submitter.form = form;
  form.appendChild(submitter);

  return { document, form, editor, submitter };
}

function editorRef(setup: {
  form: FakeElement;
  editor: FakeElement;
  kind: "textarea" | "text-input" | "contenteditable";
}): EditorRef {
  return {
    kind: setup.kind,
    element: setup.editor as unknown as HTMLElement,
    form: setup.form as unknown as HTMLFormElement,
    token: objectToken(setup.editor, "editor"),
  };
}

function makeStore(): SnapshotStore {
  return new SnapshotStore();
}

describe("opaque tokens", () => {
  test("snapshot ids are unique and monotonic", () => {
    const first = newSnapshotId();
    const second = newSnapshotId();
    assert.notEqual(first, second);
    assert.match(first, /^snap_\d+_[0-9a-f]+$/);
    const counters = tokenCountersForTest();
    assert.ok(counters.snapshots >= 2);
  });

  test("object tokens are stable per object and distinct per object", () => {
    const left = {};
    const right = {};
    assert.equal(objectToken(left, "form"), objectToken(left, "form"));
    assert.notEqual(objectToken(left, "form"), objectToken(right, "form"));
  });
});

describe("snapshot store", () => {
  test("creates a valid snapshot and returns source-free metadata", () => {
    const setup = setupNative({ value: "exact source" });
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const metadata = store.create(IDENTITY, ref, "form-1", "doc-1", "exact source");
    assert.ok(metadata !== undefined);
    assert.equal(metadata.status, "valid");
    assert.equal(metadata.editorKind, "textarea");
    assert.equal(metadata.invalidationReason, undefined);
    assert.equal(JSON.stringify(metadata).includes("exact source"), false);
    assert.equal(store.pending().length, 1);
    assert.deepEqual(store.counts(), {
      total: 1,
      valid: 1,
      invalidated: 0,
      released: 0,
      retainedSourceChars: "exact source".length,
    });
  });

  test("exact source equality, not normalization", () => {
    const setup = setupNative({ value: "identité 😀 日本語\ntoken" });
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const metadata = store.create(
      IDENTITY,
      ref,
      "form-1",
      "doc-1",
      "identité 😀 日本語\ntoken",
    );
    assert.ok(metadata !== undefined);
    assert.equal(
      store.sourceEquals(
        metadata.snapshotId,
        "doc-1",
        "identité 😀 日本語\ntoken",
        readEditor,
        isEditorCurrent,
      ),
      true,
    );
    assert.equal(
      store.sourceEquals(
        metadata.snapshotId,
        "doc-1",
        "identité 😀 日本語 token",
        readEditor,
        isEditorCurrent,
      ),
      false,
    );
    assert.equal(store.status(metadata.snapshotId), "invalidated");
  });

  test("invalidation is sticky across A-to-B-to-A", () => {
    const setup = setupNative({ value: "A" });
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const metadata = store.create(IDENTITY, ref, "form-1", "doc-1", "A");
    assert.ok(metadata !== undefined);
    setup.editor.value = "B";
    assert.equal(
      store.verify(metadata.snapshotId, "doc-1", readEditor, isEditorCurrent),
      false,
    );
    assert.equal(store.status(metadata.snapshotId), "invalidated");
    assert.equal(
      store.metadata(metadata.snapshotId)?.invalidationReason,
      "source_changed",
    );
    setup.editor.value = "A";
    assert.equal(
      store.verify(metadata.snapshotId, "doc-1", readEditor, isEditorCurrent),
      false,
    );
    assert.equal(store.status(metadata.snapshotId), "invalidated");
  });

  test("release is explicit and cannot be repeated or revived", () => {
    const setup = setupNative();
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const metadata = store.create(IDENTITY, ref, "form-1", "doc-1", "fixture source");
    assert.ok(metadata !== undefined);
    assert.equal(store.release(metadata.snapshotId), true);
    assert.equal(store.status(metadata.snapshotId), "released");
    assert.equal(
      store.metadata(metadata.snapshotId)?.invalidationReason,
      "released",
    );
    assert.equal(store.release(metadata.snapshotId), false);
    assert.equal(
      store.verify(metadata.snapshotId, "doc-1", readEditor, isEditorCurrent),
      false,
    );
  });

  test("storage stays bounded and identifiers are never reused", () => {
    const setup = setupNative();
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const identifiers = new Set<string>();
    for (let index = 0; index < MAX_SNAPSHOTS + 20; index += 1) {
      const metadata = store.create(
        IDENTITY,
        ref,
        "form-1",
        "doc-1",
        `source-${index}`,
      );
      assert.ok(metadata !== undefined);
      identifiers.add(metadata.snapshotId);
    }
    assert.equal(store.size <= MAX_SNAPSHOTS, true);
    assert.ok(store.size > 0);
    assert.equal(identifiers.size, MAX_SNAPSHOTS + 20);
    assert.equal(store.counts().total <= MAX_SNAPSHOTS, true);
  });

  test("document mismatch invalidates without reading source", () => {
    const setup = setupNative();
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const metadata = store.create(IDENTITY, ref, "form-1", "doc-1", "a");
    assert.ok(metadata !== undefined);
    assert.equal(
      store.verify(metadata.snapshotId, "doc-2", readEditor, isEditorCurrent),
      false,
    );
    assert.equal(
      store.metadata(metadata.snapshotId)?.invalidationReason,
      "document_replaced",
    );
  });

  test("successful replacement invalidates the original snapshot", () => {
    const setup = setupNative({ value: "old" });
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const metadata = store.create(IDENTITY, ref, "form-1", "doc-1", "old");
    assert.ok(metadata !== undefined);
    const result = store.replace(metadata.snapshotId, "new 😀\nline", replaceEditor);
    assert.deepEqual(result, { status: "replaced", readBackVerified: true });
    assert.equal(store.status(metadata.snapshotId), "invalidated");
    assert.equal(
      store.metadata(metadata.snapshotId)?.invalidationReason,
      "replaced",
    );
    assert.equal(setup.editor.value, "new 😀\nline");
  });

  test("single-line input newline normalization is a read-back failure", () => {
    const setup = setupNative({ editorTag: "input", value: "old" });
    const ref = editorRef({ ...setup, kind: "text-input" });
    const store = makeStore();
    const metadata = store.create(IDENTITY, ref, "form-1", "doc-1", "old");
    assert.ok(metadata !== undefined);
    const result = store.replace(metadata.snapshotId, "a\nb", replaceEditor);
    assert.deepEqual(result, { status: "failed", reason: "read_back_mismatch" });
    assert.equal(setup.editor.value, "ab");
    assert.equal(store.status(metadata.snapshotId), "invalidated");
  });

  test("detached editor replacement fails", () => {
    const setup = setupNative({ value: "old" });
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const metadata = store.create(IDENTITY, ref, "form-1", "doc-1", "old");
    assert.ok(metadata !== undefined);
    setup.form.removeChild(setup.editor);
    const result = store.replace(metadata.snapshotId, "new", replaceEditor);
    assert.deepEqual(result, { status: "failed", reason: "editor_detached" });
  });

  test("focus reaches the editor", () => {
    const setup = setupNative();
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const metadata = store.create(IDENTITY, ref, "form-1", "doc-1", "fixture source");
    assert.ok(metadata !== undefined);
    assert.equal(
      store.focus(metadata.snapshotId, "doc-1", (editor) => editor.element.focus(), isEditorCurrent),
      true,
    );
    assert.equal(setup.document.activeElement, setup.editor);
  });
});

describe("snapshot source bounds", () => {
  test("oversized source is rejected without retaining source or leaving a pending editor", () => {
    const setup = setupNative();
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const oversized = "a".repeat(MAX_SNAPSHOT_SOURCE_LENGTH + 1);
    const metadata = store.create(
      IDENTITY,
      ref,
      "form-1",
      "doc-1",
      oversized,
    );
    assert.equal(metadata, undefined);
    assert.equal(store.size, 0);
    assert.equal(store.counts().retainedSourceChars, 0);
    assert.equal(store.pendingForEditor(ref.token), undefined);
    assert.deepEqual(store.pending(), []);
  });

  test("aggregate retained source is bounded and evicts oldest valid snapshots", () => {
    const setup = setupNative();
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const chunk = "a".repeat(MAX_SNAPSHOT_SOURCE_LENGTH);
    const created: SnapshotMetadata[] = [];
    for (let index = 0; index < 5; index += 1) {
      const metadata = store.create(
        IDENTITY,
        ref,
        "form-1",
        "doc-1",
        chunk,
      );
      assert.ok(metadata !== undefined);
      created.push(metadata);
    }
    const counts = store.counts();
    assert.ok(counts.retainedSourceChars <= MAX_RETAINED_SOURCE_CHARS);
    assert.ok(counts.total < 5);
    const first = created[0];
    assert.ok(first !== undefined);
    assert.equal(store.status(first.snapshotId), "unknown");
    assert.notEqual(store.pendingForEditor(ref.token), undefined);
  });

  test("eviction releases only the evicted editor's pending state", () => {
    const setupA = setupNative();
    const refA = editorRef({ ...setupA, kind: "textarea" });
    const setupB = setupNative();
    const refB = editorRef({ ...setupB, kind: "textarea" });
    const store = makeStore();
    const firstA = store.create(
      IDENTITY,
      refA,
      "form-a",
      "doc-1",
      "a".repeat(MAX_SNAPSHOT_SOURCE_LENGTH),
    );
    assert.ok(firstA !== undefined);
    for (let index = 0; index < 4; index += 1) {
      const metadata = store.create(
        IDENTITY,
        refB,
        "form-b",
        "doc-1",
        "b".repeat(MAX_SNAPSHOT_SOURCE_LENGTH),
      );
      assert.ok(metadata !== undefined);
    }
    assert.equal(store.status(firstA.snapshotId), "unknown");
    assert.equal(store.pendingForEditor(refA.token), undefined);
    assert.notEqual(store.pendingForEditor(refB.token), undefined);
    const replacement = store.create(
      IDENTITY,
      refA,
      "form-a",
      "doc-1",
      "new A",
    );
    assert.ok(replacement !== undefined);
  });

  test("release and invalidation clear retained source and pending state", () => {
    const setup = setupNative();
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const first = store.create(
      IDENTITY,
      ref,
      "form-1",
      "doc-1",
      "a".repeat(1000),
    );
    assert.ok(first !== undefined);
    assert.equal(store.counts().retainedSourceChars, 1000);
    assert.equal(store.invalidate(first.snapshotId, "source_changed"), true);
    assert.equal(store.counts().retainedSourceChars, 0);
    assert.equal(store.pendingForEditor(ref.token), undefined);

    const second = store.create(
      IDENTITY,
      ref,
      "form-1",
      "doc-1",
      "b".repeat(500),
    );
    assert.ok(second !== undefined);
    assert.equal(store.release(second.snapshotId), true);
    assert.equal(store.counts().retainedSourceChars, 0);
    assert.equal(store.pendingForEditor(ref.token), undefined);
  });
});

describe("native form adapter", () => {
  test("recognizes a marked textarea form", () => {
    const setup = setupNative();
    const adapter = new NativeFormAdapter();
    assert.equal(
      adapter.recognizes(setup.form as unknown as HTMLFormElement),
      true,
    );
    const coverage = adapter.detect(setup.form as unknown as HTMLFormElement);
    assert.equal(coverage.status, "supported");
    if (coverage.status !== "supported") return;
    assert.equal(coverage.identity.id, "cribra.native-form");
    assert.equal(coverage.editor.kind, "textarea");
    assert.equal(coverage.formToken.length > 0, true);
  });

  test("recognizes an ordinary text input", () => {
    const setup = setupNative({ editorTag: "input" });
    const coverage = new NativeFormAdapter().detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "supported");
    if (coverage.status !== "supported") return;
    assert.equal(coverage.editor.kind, "text-input");
  });

  test("does not recognize an unmarked form", () => {
    const setup = setupNative({ marker: null });
    const coverage = new NativeFormAdapter().detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.deepEqual(coverage, {
      status: "unsupported",
      identity: { id: "cribra.native-form", version: "1.0.0" },
      reason: "unrecognized_form",
    });
  });

  test("rejects password and hidden designated editors", () => {
    for (const type of ["password", "hidden"]) {
      const setup = setupNative({
        editorTag: "input",
        editorType: type,
      });
      const coverage = new NativeFormAdapter().detect(
        setup.form as unknown as HTMLFormElement,
      );
      assert.equal(coverage.status, "unsupported");
      if (coverage.status !== "unsupported") return;
      assert.equal(coverage.reason, "password_or_hidden_field");
    }
  });

  test("rejects multiple designated editors and missing editors", () => {
    const multiple = setupNative();
    const second = multiple.document.createElement("textarea");
    second.setAttribute(EDITOR_MARKER, EDITOR_VALUE);
    multiple.form.appendChild(second);
    const multipleCoverage = new NativeFormAdapter().detect(
      multiple.form as unknown as HTMLFormElement,
    );
    assert.equal(multipleCoverage.status, "unsupported");
    if (multipleCoverage.status === "unsupported") {
      assert.equal(multipleCoverage.reason, "multiple_eligible_editors");
    }

    const missing = setupNative();
    missing.form.removeChild(missing.editor);
    const missingCoverage = new NativeFormAdapter().detect(
      missing.form as unknown as HTMLFormElement,
    );
    assert.equal(missingCoverage.status, "unsupported");
    if (missingCoverage.status === "unsupported") {
      assert.equal(missingCoverage.reason, "no_eligible_editor");
    }
  });

  test("does not claim contenteditable coverage", () => {
    const setup = setupContentEditable();
    const coverage = new NativeFormAdapter().detect(
      setup.form as unknown as HTMLFormElement,
    );
    // The native adapter does not recognize the contenteditable marker.
    assert.equal(coverage.status, "unsupported");
    if (coverage.status === "unsupported") {
      assert.equal(coverage.reason, "unrecognized_form");
    }
  });

  test("reports read-only and disabled editors as degraded", () => {
    for (const options of [{ readOnly: true }, { disabled: true }]) {
      const setup = setupNative(options);
      const coverage = new NativeFormAdapter().detect(
        setup.form as unknown as HTMLFormElement,
      );
      assert.equal(coverage.status, "degraded");
      if (coverage.status === "degraded") {
        assert.equal(
          coverage.reason,
          options.readOnly ? "editor_readonly" : "editor_disabled",
        );
      }
    }
  });

  test("reports detached forms as invalidated and detached editors as unsupported at adapter level", () => {
    const detachedEditor = setupNative();
    detachedEditor.form.removeChild(detachedEditor.editor);
    const editorCoverage = new NativeFormAdapter().detect(
      detachedEditor.form as unknown as HTMLFormElement,
    );
    assert.equal(editorCoverage.status, "unsupported");

    const detachedForm = setupNative();
    detachedForm.document.removeChild(detachedForm.form);
    const formCoverage = new NativeFormAdapter().detect(
      detachedForm.form as unknown as HTMLFormElement,
    );
    assert.equal(formCoverage.status, "invalidated");
    if (formCoverage.status === "invalidated") {
      assert.equal(formCoverage.reason, "form_detached");
    }
  });

  test("accepts implicit and same-form submitters only", () => {
    const setup = setupNative();
    const adapter = new NativeFormAdapter();
    const coverage = adapter.detect(setup.form as unknown as HTMLFormElement);
    assert.equal(coverage.status, "supported");
    if (coverage.status !== "supported") return;
    assert.equal(adapter.supportsSubmitter(coverage, null), true);
    assert.equal(
      adapter.supportsSubmitter(
        coverage,
        setup.submitter as unknown as HTMLElement,
      ),
      true,
    );
    const other = setup.document.createElement("button");
    other.type = "submit";
    assert.equal(
      adapter.supportsSubmitter(coverage, other as unknown as HTMLElement),
      false,
    );
    const plain = setup.document.createElement("div");
    assert.equal(
      adapter.supportsSubmitter(coverage, plain as unknown as HTMLElement),
      false,
    );
  });
});

describe("contenteditable adapter", () => {
  test("recognizes plain-text contenteditable and reads exact textContent", () => {
    const setup = setupContentEditable({ value: "identité 😀\n日本語" });
    const adapter = new ContentEditableAdapter();
    const coverage = adapter.detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "supported");
    if (coverage.status !== "supported") return;
    assert.equal(coverage.editor.kind, "contenteditable");
    assert.equal(adapter.read(coverage), "identité 😀\n日本語");
  });

  test("replaces through textContent and verifies exact read-back", () => {
    const setup = setupContentEditable({ value: "old" });
    const adapter = new ContentEditableAdapter();
    const coverage = adapter.detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "supported");
    if (coverage.status !== "supported") return;
    const result = adapter.replace(coverage, "😀 line one\nline two");
    assert.deepEqual(result, { status: "replaced", readBackVerified: true });
    assert.equal(setup.editor.textContent, "😀 line one\nline two");
  });

  test("reports contenteditable=false as degraded, not supported", () => {
    const setup = setupContentEditable({ contenteditable: "false" });
    const coverage = new ContentEditableAdapter().detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "degraded");
    if (coverage.status === "degraded") {
      assert.equal(coverage.reason, "editor_not_editable");
    }
  });

  test("does not claim textarea coverage", () => {
    const setup = setupNative();
    const coverage = new ContentEditableAdapter().detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "unsupported");
    if (coverage.status === "unsupported") {
      assert.equal(coverage.reason, "unrecognized_form");
    }
  });
});

describe("adapter registry", () => {
  test("leaves an unrelated unsupported form untouched", () => {
    const setup = setupNative({ marker: null });
    const registry = new AdapterRegistry();
    const coverage = registry.detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "unsupported");
    if (coverage.status === "unsupported") {
      assert.equal(coverage.reason, "unrecognized_form");
    }
    assert.equal(
      registry.isKnown(setup.form as unknown as HTMLFormElement),
      false,
    );
  });

  test("a known form with a replaced editor becomes invalidated", () => {
    const setup = setupNative();
    const registry = new AdapterRegistry();
    assert.equal(
      registry.detect(setup.form as unknown as HTMLFormElement).status,
      "supported",
    );
    const replacement = setup.document.createElement("textarea");
    replacement.setAttribute(EDITOR_MARKER, EDITOR_VALUE);
    replacement.value = "replacement";
    setup.editor.replaceWith(replacement);
    const coverage = registry.detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "invalidated");
    if (coverage.status === "invalidated") {
      assert.equal(coverage.reason, "editor_replaced");
    }
  });

  test("a known form whose editor vanishes becomes invalidated", () => {
    const setup = setupNative();
    const registry = new AdapterRegistry();
    assert.equal(
      registry.detect(setup.form as unknown as HTMLFormElement).status,
      "supported",
    );
    setup.form.removeChild(setup.editor);
    const coverage = registry.detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "invalidated");
    if (coverage.status === "invalidated") {
      assert.equal(coverage.reason, "adapter_invalidated");
      assert.equal(coverage.cause, "no_eligible_editor");
    }
  });

  test("a known form whose marker vanishes becomes invalidated", () => {
    const setup = setupNative();
    const registry = new AdapterRegistry();
    assert.equal(
      registry.detect(setup.form as unknown as HTMLFormElement).status,
      "supported",
    );
    setup.form.removeAttribute(NATIVE_FORM_MARKER);
    const coverage = registry.detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "invalidated");
    if (coverage.status === "invalidated") {
      assert.equal(coverage.reason, "adapter_invalidated");
      assert.equal(coverage.cause, "unrecognized_form");
    }
  });

  test("a known read-only editor becomes degraded", () => {
    const setup = setupNative();
    const registry = new AdapterRegistry();
    assert.equal(
      registry.detect(setup.form as unknown as HTMLFormElement).status,
      "supported",
    );
    setup.editor.readOnly = true;
    const coverage = registry.detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "degraded");
    if (coverage.status === "degraded") {
      assert.equal(coverage.reason, "editor_readonly");
    }
  });

  test("loss reports are deduplicated", () => {
    const registry = new AdapterRegistry();
    const report = {
      kind: "coverage_lost" as const,
      identity: IDENTITY,
      formToken: "form-1",
      reason: "form_replaced" as const,
      at: 1,
    };
    assert.equal(registry.recordLoss(report), true);
    assert.equal(registry.recordLoss(report), false);
    assert.equal(registry.losses().length, 1);
  });

  test("an evicted but still-connected protected form is blocked, not re-authorized", () => {
    const document = new FakeDocument();
    const registry = new AdapterRegistry();
    const forms: FakeElement[] = [];
    for (let index = 0; index < 70; index += 1) {
      const form = document.createElement("form");
      form.setAttribute(NATIVE_FORM_MARKER, NATIVE_FORM_VALUE);
      document.appendChild(form);
      const editor = document.createElement("textarea");
      editor.setAttribute(EDITOR_MARKER, EDITOR_VALUE);
      editor.value = `source-${index}`;
      form.appendChild(editor);
      const submitter = document.createElement("button");
      submitter.type = "submit";
      submitter.form = form;
      form.appendChild(submitter);
      forms.push(form);
      assert.equal(
        registry.detect(form as unknown as HTMLFormElement).status,
        "supported",
      );
    }
    const first = forms[0];
    assert.ok(first !== undefined);
    const coverage = registry.detect(first as unknown as HTMLFormElement);
    assert.equal(coverage.status, "invalidated");
    if (coverage.status === "invalidated") {
      assert.equal(coverage.reason, "adapter_invalidated");
    }
    assert.equal(
      registry.isKnown(first as unknown as HTMLFormElement),
      true,
    );
  });

  test("disposal makes detection unsupported", () => {
    const setup = setupNative();
    const registry = new AdapterRegistry();
    registry.dispose();
    const coverage = registry.detect(
      setup.form as unknown as HTMLFormElement,
    );
    assert.equal(coverage.status, "unsupported");
    if (coverage.status === "unsupported") {
      assert.equal(coverage.reason, "adapter_disposed");
    }
  });
});

interface InterceptorHarness {
  readonly interceptor: SubmissionInterceptor;
  readonly store: SnapshotStore;
  readonly setup: NativeSetup;
}

function makeInterceptor(
  options: {
    readonly setup?: NativeSetup;
    readonly value?: string;
  } = {},
): InterceptorHarness {
  const setup = options.setup ?? setupNative({ value: options.value });
  const store = new SnapshotStore();
  const interceptor = new SubmissionInterceptor({
    document: setup.document as unknown as Document,
    store,
    now: () => 1_000,
    observerFactory: (callback) =>
      new FakeMutationObserver(callback) as unknown as MutationObserver,
  });
  return { interceptor, store, setup };
}

describe("submission interceptor", () => {
  test("captures exact source and blocks the native submission", () => {
    const { interceptor, store, setup } = makeInterceptor({
      value: "identité 😀 日本語\ntoken",
    });
    const event = submitEvent(setup.form, { submitter: setup.submitter });
    interceptor.handleSubmit(event as unknown as SubmitEvent);
    assert.equal(event.defaultPrevented, true);
    assert.equal(event.propagationStopped, true);
    const history = interceptor.history();
    assert.equal(history.length, 1);
    const capture = history[0];
    assert.equal(capture?.kind, "captured");
    if (capture?.kind !== "captured") return;
    assert.equal(capture.submitterKind, "submit");
    assert.equal(capture.editorKind, "textarea");
    assert.equal(capture.blocked, true);
    assert.equal(capture.cancelled, true);
    assert.equal(store.pending().length, 1);
    assert.equal(
      interceptor.sourceEquals(
        capture.snapshotId,
        "identité 😀 日本語\ntoken",
      ),
      true,
    );
    assert.equal(
      JSON.stringify(history).includes("identité 😀 日本語\ntoken"),
      false,
    );
  });

  test("suppresses a duplicate pending attempt without a second snapshot", () => {
    const { interceptor, store, setup } = makeInterceptor();
    const first = submitEvent(setup.form);
    interceptor.handleSubmit(first as unknown as SubmitEvent);
    const firstCapture = interceptor.history()[0];
    assert.equal(firstCapture?.kind, "captured");
    if (firstCapture?.kind !== "captured") return;

    const second = submitEvent(setup.form);
    interceptor.handleSubmit(second as unknown as SubmitEvent);
    assert.equal(second.defaultPrevented, true);
    assert.equal(interceptor.history()[1]?.kind, "duplicate");
    const duplicate = interceptor.history()[1];
    if (duplicate?.kind === "duplicate") {
      assert.equal(duplicate.snapshotId, firstCapture.snapshotId);
    }
    assert.equal(store.pending().length, 1);
    assert.equal(interceptor.counts().captures, 1);
    assert.equal(interceptor.counts().duplicates, 1);
  });

  test("an explicit release permits a new capture with a new id", () => {
    const { interceptor, setup } = makeInterceptor();
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    const first = interceptor.history()[0];
    assert.equal(first?.kind, "captured");
    if (first?.kind !== "captured") return;
    assert.equal(interceptor.releaseSnapshot(first.snapshotId), true);
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    const second = interceptor.history()[1];
    assert.equal(second?.kind, "captured");
    if (second?.kind !== "captured") return;
    assert.notEqual(second.snapshotId, first.snapshotId);
  });

  test("non-cancellable events are reported and never claimed as blocked", () => {
    const { interceptor, setup } = makeInterceptor();
    const event = submitEvent(setup.form, { cancelable: false });
    interceptor.handleSubmit(event as unknown as SubmitEvent);
    assert.equal(event.defaultPrevented, false);
    assert.equal(event.propagationStopped, false);
    const result = interceptor.history()[0];
    assert.equal(result?.kind, "not_cancellable");
    if (result?.kind === "not_cancellable") {
      assert.equal(result.blocked, false);
      assert.equal(result.cancelled, false);
    }
    assert.equal(interceptor.pending().length, 0);
    assert.equal(interceptor.counts().notCancellable, 1);
  });

  test("unrelated unsupported forms are untouched", () => {
    const setup = setupNative({ marker: null });
    const { interceptor } = makeInterceptor({ setup });
    const event = submitEvent(setup.form);
    interceptor.handleSubmit(event as unknown as SubmitEvent);
    assert.equal(event.defaultPrevented, false);
    assert.equal(event.propagationStopped, false);
    assert.equal(interceptor.history()[0]?.kind, "unsupported_ignored");
    assert.equal(interceptor.counts().unsupportedIgnored, 1);
  });

  test("a known form with a removed editor blocks and reports invalidation", () => {
    const { interceptor, setup } = makeInterceptor();
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    setup.form.removeChild(setup.editor);
    const event = submitEvent(setup.form);
    interceptor.handleSubmit(event as unknown as SubmitEvent);
    assert.equal(event.defaultPrevented, true);
    const result = interceptor.history()[1];
    assert.equal(result?.kind, "invalidated");
    if (result?.kind === "invalidated") {
      assert.equal(result.reason, "adapter_invalidated");
      assert.equal(result.cause, "no_eligible_editor");
    }
    assert.equal(interceptor.counts().losses >= 1, true);
  });

  test("editor replacement blocks rather than silently re-authorizing", () => {
    const { interceptor, setup } = makeInterceptor();
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    const replacement = setup.document.createElement("textarea");
    replacement.setAttribute(EDITOR_MARKER, EDITOR_VALUE);
    replacement.value = "replacement";
    setup.editor.replaceWith(replacement);
    const event = submitEvent(setup.form);
    interceptor.handleSubmit(event as unknown as SubmitEvent);
    assert.equal(event.defaultPrevented, true);
    const result = interceptor.history()[1];
    assert.equal(result?.kind, "invalidated");
    if (result?.kind === "invalidated") {
      assert.equal(result.reason, "editor_replaced");
    }
  });

  test("a degraded known editor blocks and reports degradation", () => {
    const { interceptor, setup } = makeInterceptor();
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    setup.editor.readOnly = true;
    const event = submitEvent(setup.form);
    interceptor.handleSubmit(event as unknown as SubmitEvent);
    assert.equal(event.defaultPrevented, true);
    const result = interceptor.history()[1];
    assert.equal(result?.kind, "degraded");
    if (result?.kind === "degraded") {
      assert.equal(result.reason, "editor_readonly");
    }
  });

  test("verification detects observable and silent source changes", () => {
    const { interceptor, setup } = makeInterceptor({ value: "A" });
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    const first = interceptor.history()[0];
    assert.equal(first?.kind, "captured");
    if (first?.kind !== "captured") return;

    setup.editor.value = "B";
    assert.equal(interceptor.verifySnapshot(first.snapshotId), false);
    assert.equal(interceptor.status(first.snapshotId), "invalidated");
    setup.editor.value = "A";
    assert.equal(interceptor.verifySnapshot(first.snapshotId), false);
    assert.equal(interceptor.status(first.snapshotId), "invalidated");
  });

  test("replacement succeeds and invalidates the previous snapshot", () => {
    const { interceptor, setup } = makeInterceptor({ value: "old" });
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    const first = interceptor.history()[0];
    assert.equal(first?.kind, "captured");
    if (first?.kind !== "captured") return;
    const result = interceptor.replaceSnapshot(
      first.snapshotId,
      "new 😀\nline",
    );
    assert.deepEqual(result, { status: "replaced", readBackVerified: true });
    assert.equal(setup.editor.value, "new 😀\nline");
    assert.equal(interceptor.status(first.snapshotId), "invalidated");
    assert.equal(
      interceptor.metadata(first.snapshotId)?.invalidationReason,
      "replaced",
    );
  });

  test("text input newline normalization is not reported as success", () => {
    const setup = setupNative({ editorTag: "input", value: "old" });
    const { interceptor } = makeInterceptor({ setup });
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    const first = interceptor.history()[0];
    assert.equal(first?.kind, "captured");
    if (first?.kind !== "captured") return;
    const result = interceptor.replaceSnapshot(first.snapshotId, "a\nb");
    assert.deepEqual(result, { status: "failed", reason: "read_back_mismatch" });
    assert.equal(setup.editor.value, "ab");
    assert.equal(interceptor.status(first.snapshotId), "invalidated");
  });

  test("adapter detection exceptions fail closed", () => {
    const setup = setupNative();
    const throwingAdapter: PageAdapter = {
      identity: { id: "test.throwing-detect", version: "1.0.0" },
      recognizes: () => true,
      detect: () => {
        throw new Error("detect failure");
      },
      supportsSubmitter: () => true,
      read: () => "source",
      replace: () => ({ status: "failed", reason: "write_failed" }),
      focus: () => {},
    };
    const registry = new AdapterRegistry({ adapters: [throwingAdapter] });
    const interceptor = new SubmissionInterceptor({
      document: setup.document as unknown as Document,
      registry,
      now: () => 1_000,
      observerFactory: (callback) =>
        new FakeMutationObserver(callback) as unknown as MutationObserver,
    });
    const event = submitEvent(setup.form);
    interceptor.handleSubmit(event as unknown as SubmitEvent);
    assert.equal(event.defaultPrevented, true);
    assert.equal(event.propagationStopped, true);
    const result = interceptor.history()[0];
    assert.equal(result?.kind, "invalidated");
    if (result?.kind === "invalidated") {
      assert.equal(result.reason, "adapter_invalidated");
    }
    assert.equal(interceptor.pending().length, 0);
  });

  test("adapter read exceptions fail closed after cancellation", () => {
    const setup = setupNative();
    const ref = editorRef({ ...setup, kind: "textarea" });
    let coverage: SupportedCoverage | undefined;
    const throwingAdapter: PageAdapter = {
      identity: { id: "test.throwing-read", version: "1.0.0" },
      recognizes: () => true,
      detect: (): AdapterCoverage => {
        if (coverage === undefined) throw new Error("coverage unavailable");
        return coverage;
      },
      supportsSubmitter: () => true,
      read: () => {
        throw new Error("read failure");
      },
      replace: () => ({ status: "failed", reason: "write_failed" }),
      focus: () => {},
    };
    coverage = {
      status: "supported",
      identity: throwingAdapter.identity,
      adapter: throwingAdapter,
      editor: ref,
      formToken: objectToken(setup.form, "form"),
    };
    const registry = new AdapterRegistry({ adapters: [throwingAdapter] });
    const interceptor = new SubmissionInterceptor({
      document: setup.document as unknown as Document,
      registry,
      now: () => 1_000,
      observerFactory: (callback) =>
        new FakeMutationObserver(callback) as unknown as MutationObserver,
    });
    const event = submitEvent(setup.form);
    interceptor.handleSubmit(event as unknown as SubmitEvent);
    assert.equal(event.defaultPrevented, true);
    assert.equal(event.propagationStopped, true);
    const result = interceptor.history()[0];
    assert.equal(result?.kind, "invalidated");
    if (result?.kind === "invalidated") {
      assert.equal(result.reason, "adapter_invalidated");
    }
    assert.equal(interceptor.pending().length, 0);
  });

  test("oversized intercepted content remains blocked", () => {
    const setup = setupNative({
      value: "a".repeat(MAX_SNAPSHOT_SOURCE_LENGTH + 1),
    });
    const { interceptor } = makeInterceptor({ setup });
    const event = submitEvent(setup.form);
    interceptor.handleSubmit(event as unknown as SubmitEvent);
    assert.equal(event.defaultPrevented, true);
    assert.equal(event.propagationStopped, true);
    const result = interceptor.history()[0];
    assert.equal(result?.kind, "invalidated");
    if (result?.kind === "invalidated") {
      assert.equal(result.reason, "oversized_source");
    }
    assert.equal(interceptor.pending().length, 0);
    assert.equal(interceptor.counts().retainedSourceChars, 0);
  });

  test("persisted pagehide suspends without disabling interception", () => {
    const { interceptor, setup } = makeInterceptor({ value: "A" });
    interceptor.install();
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    const first = interceptor.history()[0];
    assert.equal(first?.kind, "captured");
    if (first?.kind !== "captured") return;

    interceptor.handlePageHide({ persisted: true } as PageTransitionEvent);
    assert.equal(interceptor.isInstalled(), true);
    assert.equal(interceptor.pending().length, 0);
    assert.equal(interceptor.status(first.snapshotId), "invalidated");
    assert.equal(
      interceptor.metadata(first.snapshotId)?.invalidationReason,
      "suspended",
    );
    assert.equal(interceptor.counts().suspensions, 1);
    assert.equal(interceptor.counts().retainedSourceChars, 0);

    interceptor.handlePageShow({ persisted: true } as PageTransitionEvent);
    assert.equal(interceptor.isInstalled(), true);
    assert.equal(interceptor.counts().restorations, 1);
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    const second = interceptor.history()[1];
    assert.equal(second?.kind, "captured");
    if (second?.kind === "captured") {
      assert.notEqual(second.snapshotId, first.snapshotId);
    }

    interceptor.handlePageHide({ persisted: false } as PageTransitionEvent);
    assert.equal(interceptor.isInstalled(), false);
    interceptor.dispose();
  });

  test("disposed interceptor stops capturing and clears pending state", () => {
    const { interceptor, setup } = makeInterceptor();
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    assert.equal(interceptor.pending().length, 1);
    interceptor.dispose();
    assert.equal(interceptor.isInstalled(), false);
    assert.equal(interceptor.pending().length, 0);
    const event = submitEvent(setup.form);
    interceptor.handleSubmit(event as unknown as SubmitEvent);
    assert.equal(event.defaultPrevented, false);
    assert.equal(interceptor.history().length, 0);
    assert.equal(interceptor.verifySnapshot("anything"), false);
    assert.deepEqual(interceptor.replaceSnapshot("anything", "x"), {
      status: "failed",
      reason: "snapshot_unavailable",
    });
  });

  test("the qualification hook exposes source-free metadata only", () => {
    const { interceptor, setup } = makeInterceptor({
      value: "sentinel-value-123",
    });
    interceptor.install();
    installQualificationHook(interceptor);
    const hook = (
      globalThis as unknown as {
        __cribraGuardQualification?: {
          installed: () => boolean;
          captures: () => readonly unknown[];
          verify: (id: string) => boolean;
          sourceEquals: (id: string, candidate: string) => boolean;
          replace: (id: string, text: string) => ReplacementResult;
          pending: () => readonly string[];
          counts: () => Record<string, number>;
          dispose: () => void;
        };
      }
    ).__cribraGuardQualification;
    assert.ok(hook !== undefined);
    if (hook === undefined) return;
    assert.equal(hook.installed(), true);
    interceptor.handleSubmit(submitEvent(setup.form) as unknown as SubmitEvent);
    const capture = hook.captures()[0] as { kind: string; snapshotId: string };
    assert.equal(capture.kind, "captured");
    assert.equal(JSON.stringify(hook.captures()).includes("sentinel-value-123"), false);
    assert.equal(hook.sourceEquals(capture.snapshotId, "sentinel-value-123"), true);
    assert.equal(JSON.stringify(hook.counts()).includes("sentinel-value-123"), false);
    assert.equal(hook.pending().length, 1);
    const replacement = hook.replace(capture.snapshotId, "replacement");
    assert.deepEqual(replacement, { status: "replaced", readBackVerified: true });
    assert.equal(hook.verify(capture.snapshotId), false);
    hook.dispose();
    assert.equal(hook.pending().length, 0);
    delete (globalThis as Record<string, unknown>)
      .__cribraGuardQualification;
  });
});

describe("snapshot metadata safety", () => {
  test("metadata never carries source or live DOM references", () => {
    const setup = setupNative({ value: "secret source" });
    const ref = editorRef({ ...setup, kind: "textarea" });
    const store = makeStore();
    const metadata = store.create(IDENTITY, ref, "form-1", "doc-1", "secret source");
    assert.ok(metadata !== undefined);
    const serialized = JSON.stringify(metadata as SnapshotMetadata);
    assert.equal(serialized.includes("secret source"), false);
    assert.equal(serialized.includes('"element"'), false);
    assert.equal(serialized.includes('"source"'), false);
  });
});
