/**
 * Editor read, write, focus, and verification operations.
 *
 * Native text controls are written through the prototype value setter so
 * framework-installed instance setters are not relied on. Contenteditable text
 * is written only through `textContent`; `innerHTML` is never used.
 *
 * A replacement is successful only when the exact requested text reads back.
 * Failed, detached, read-only, or normalized writes are never reported as
 * success.
 */

import type {
  EditorKind,
  EditorRef,
  ReplacementFailureReason,
  ReplacementResult,
  SubmitterKind,
} from "./contract.js";

/** Deliberately narrow: only ordinary single-line text controls. */
const TEXT_INPUT_TYPES = new Set(["text", "search"]);

function failed(reason: ReplacementFailureReason): ReplacementResult {
  return { status: "failed", reason };
}

export function isFormElement(value: unknown): value is HTMLFormElement {
  return (
    value !== null &&
    typeof value === "object" &&
    (value as { tagName?: unknown }).tagName === "FORM" &&
    typeof (value as { querySelectorAll?: unknown }).querySelectorAll ===
      "function"
  );
}

export function formFromEventTarget(target: unknown): HTMLFormElement | null {
  if (isFormElement(target)) return target;
  if (
    target !== null &&
    typeof target === "object" &&
    typeof (target as { closest?: unknown }).closest === "function"
  ) {
    const form = (target as { closest: (selector: string) => unknown }).closest(
      "form",
    );
    return isFormElement(form) ? form : null;
  }
  return null;
}

export function isTextAreaControl(element: HTMLElement): boolean {
  return element.tagName === "TEXTAREA";
}

export function isTextInputControl(element: HTMLElement): boolean {
  if (element.tagName !== "INPUT") return false;
  const type = ((element as { type?: string }).type ?? "text").toLowerCase();
  return TEXT_INPUT_TYPES.has(type);
}

export function isContentEditableElement(element: HTMLElement): boolean {
  const native = (element as { isContentEditable?: boolean }).isContentEditable;
  if (native === true) return true;
  const attribute = element.getAttribute?.("contenteditable");
  return (
    attribute === "" || attribute === "true" || attribute === "plaintext-only"
  );
}

export function editorKindOf(element: HTMLElement): EditorKind | undefined {
  if (isTextAreaControl(element)) return "textarea";
  if (isTextInputControl(element)) return "text-input";
  if (isContentEditableElement(element)) return "contenteditable";
  return undefined;
}

export function isSubmitButton(element: HTMLElement): boolean {
  if (element.tagName === "BUTTON") {
    const type = ((element as { type?: string }).type ?? "submit").toLowerCase();
    return type === "submit";
  }
  if (element.tagName === "INPUT") {
    const type = ((element as { type?: string }).type ?? "").toLowerCase();
    return type === "submit" || type === "image";
  }
  return false;
}

export function isImageSubmitter(element: HTMLElement): boolean {
  return (
    element.tagName === "INPUT" &&
    ((element as { type?: string }).type ?? "").toLowerCase() === "image"
  );
}

export function submitterKindOf(submitter: HTMLElement | null): SubmitterKind {
  if (submitter === null) return "implicit";
  return isImageSubmitter(submitter) ? "image" : "submit";
}

export function readEditor(editor: EditorRef): string {
  if (editor.kind === "contenteditable") {
    return editor.element.textContent ?? "";
  }
  return (editor.element as { value?: string }).value ?? "";
}

export function isEditorCurrent(editor: EditorRef): boolean {
  return (
    editor.element.isConnected === true &&
    editor.form.isConnected === true &&
    editor.form.contains(editor.element)
  );
}

export function writabilityReason(
  editor: EditorRef,
): "read_only" | "editor_disabled" | undefined {
  const element = editor.element;
  if (editor.kind === "contenteditable") {
    return isContentEditableElement(element) ? undefined : "read_only";
  }
  if ((element as { disabled?: boolean }).disabled === true) {
    return "editor_disabled";
  }
  if ((element as { readOnly?: boolean }).readOnly === true) {
    return "read_only";
  }
  return undefined;
}

export function isEditorWritable(editor: EditorRef): boolean {
  return writabilityReason(editor) === undefined;
}

/** Maps replacement-level writability failures into coverage reasons. */
export function coverageWritabilityReason(
  editor: EditorRef,
): "editor_readonly" | "editor_disabled" | undefined {
  const reason = writabilityReason(editor);
  return reason === "read_only" ? "editor_readonly" : reason;
}

function dispatchInputAndChange(element: HTMLElement, text: string): void {
  if (typeof element.dispatchEvent !== "function") return;
  const inputEventConstructor = (
    globalThis as { InputEvent?: typeof InputEvent }
  ).InputEvent;
  const inputEvent =
    inputEventConstructor === undefined
      ? new Event("input", { bubbles: true, composed: true })
      : new inputEventConstructor("input", {
          bubbles: true,
          composed: true,
          inputType: "insertText",
          data: text,
        });
  element.dispatchEvent(inputEvent);
  element.dispatchEvent(new Event("change", { bubbles: true }));
}

export function replaceEditor(
  editor: EditorRef,
  text: string,
): ReplacementResult {
  const { element, form, kind } = editor;
  if (form.isConnected !== true) return failed("form_detached");
  if (element.isConnected !== true || form.contains(element) !== true) {
    return failed("editor_detached");
  }
  const reason = writabilityReason(editor);
  if (reason !== undefined) return failed(reason);

  try {
    if (kind === "contenteditable") {
      element.textContent = text;
    } else {
      const prototype = Object.getPrototypeOf(element) as object | null;
      const descriptor =
        prototype === null
          ? undefined
          : Object.getOwnPropertyDescriptor(prototype, "value");
      if (descriptor?.set !== undefined) {
        descriptor.set.call(element, text);
      } else {
        (element as unknown as { value: string }).value = text;
      }
    }
  } catch {
    return failed("write_failed");
  }

  if (element.isConnected !== true || form.contains(element) !== true) {
    return failed("editor_detached");
  }

  dispatchInputAndChange(element, text);

  if (element.isConnected !== true || form.contains(element) !== true) {
    return failed("editor_detached");
  }
  if (readEditor(editor) !== text) {
    return failed("read_back_mismatch");
  }
  return { status: "replaced", readBackVerified: true };
}

export function focusEditor(editor: EditorRef): void {
  if (typeof editor.element.focus === "function") editor.element.focus();
}
