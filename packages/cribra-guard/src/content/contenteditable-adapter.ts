/**
 * Qualification fixture adapter for one controlled plain-text contenteditable.
 *
 * The logical source is exact `textContent`; there is no rich-text model, no
 * closed shadow root, no canvas editor, and no framework-specific editor state.
 * Content is never injected through `innerHTML`.
 */

import type {
  AdapterCoverage,
  AdapterIdentity,
  CoverageFailureReason,
  PageAdapter,
  ReplacementResult,
  SupportedCoverage,
  UnsupportedCoverage,
} from "./contract.js";
import {
  coverageWritabilityReason,
  focusEditor,
  isContentEditableElement,
  isEditorWritable,
  isSubmitButton,
  readEditor,
  replaceEditor,
} from "./editors.js";
import { objectToken } from "./tokens.js";
import { EDITOR_MARKER, EDITOR_VALUE } from "./native-form-adapter.js";

export const CONTENTEDITABLE_ADAPTER_ID = "cribra.contenteditable";
export const CONTENTEDITABLE_MARKER = "data-cribra-adapter";
export const CONTENTEDITABLE_VALUE = "contenteditable";

const IDENTITY: AdapterIdentity = {
  id: CONTENTEDITABLE_ADAPTER_ID,
  version: "1.0.0",
};

function unsupported(reason: CoverageFailureReason): UnsupportedCoverage {
  return { status: "unsupported", identity: IDENTITY, reason };
}

export class ContentEditableAdapter implements PageAdapter {
  readonly identity = IDENTITY;

  recognizes(form: HTMLFormElement): boolean {
    return (
      form.getAttribute(CONTENTEDITABLE_MARKER) === CONTENTEDITABLE_VALUE
    );
  }

  detect(form: HTMLFormElement): AdapterCoverage {
    if (!this.recognizes(form)) return unsupported("unrecognized_form");
    if (form.isConnected !== true) {
      return { status: "invalidated", identity: IDENTITY, reason: "form_detached" };
    }

    const candidates = Array.from(
      form.querySelectorAll<HTMLElement>(
        `[${EDITOR_MARKER}="${EDITOR_VALUE}"]`,
      ),
    );
    if (candidates.length === 0) return unsupported("no_eligible_editor");
    if (candidates.length > 1) return unsupported("multiple_eligible_editors");

    const element = candidates[0];
    if (element === undefined) return unsupported("no_eligible_editor");

    if (!isContentEditableElement(element)) {
      return element.getAttribute?.("contenteditable") === "false"
        ? { status: "degraded", identity: IDENTITY, reason: "editor_not_editable" }
        : unsupported("unsupported_editor");
    }
    if (element.isConnected !== true) {
      return { status: "invalidated", identity: IDENTITY, reason: "editor_detached" };
    }
    if (form.contains(element) !== true) {
      return { status: "invalidated", identity: IDENTITY, reason: "editor_detached" };
    }

    const editor = {
      kind: "contenteditable" as const,
      element,
      form,
      token: objectToken(element, "editor"),
    };
    if (!isEditorWritable(editor)) {
      return {
        status: "degraded",
        identity: IDENTITY,
        reason: coverageWritabilityReason(editor) ?? "editor_not_editable",
      };
    }

    return {
      status: "supported",
      identity: IDENTITY,
      adapter: this,
      editor,
      formToken: objectToken(form, "form"),
    };
  }

  supportsSubmitter(
    coverage: SupportedCoverage,
    submitter: HTMLElement | null,
  ): boolean {
    if (submitter === null) return true;
    if (!isSubmitButton(submitter)) return false;
    const associated = (submitter as { form?: HTMLFormElement | null }).form;
    return (
      associated === coverage.editor.form ||
      coverage.editor.form.contains(submitter)
    );
  }

  read(coverage: SupportedCoverage): string {
    return readEditor(coverage.editor);
  }

  replace(coverage: SupportedCoverage, text: string): ReplacementResult {
    return replaceEditor(coverage.editor, text);
  }

  focus(coverage: SupportedCoverage): void {
    focusEditor(coverage.editor);
  }
}
