/**
 * Qualification fixture adapter for explicitly recognized native forms.
 *
 * A form is recognized only when it carries the fixture marker and exactly one
 * designated textarea or ordinary text input. Password inputs, hidden inputs,
 * unrelated form data, and arbitrary page text are never read.
 *
 * This adapter is part of the qualification-only content package. The
 * production extension does not register any adapter.
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
  editorKindOf,
  focusEditor,
  isEditorWritable,
  isSubmitButton,
  readEditor,
  replaceEditor,
} from "./editors.js";
import { objectToken } from "./tokens.js";

export const NATIVE_FORM_ADAPTER_ID = "cribra.native-form";
export const NATIVE_FORM_MARKER = "data-cribra-adapter";
export const NATIVE_FORM_VALUE = "native-form";
export const EDITOR_MARKER = "data-cribra-editor";
export const EDITOR_VALUE = "source";

const IDENTITY: AdapterIdentity = {
  id: NATIVE_FORM_ADAPTER_ID,
  version: "1.0.0",
};

function unsupported(reason: CoverageFailureReason): UnsupportedCoverage {
  return { status: "unsupported", identity: IDENTITY, reason };
}

export class NativeFormAdapter implements PageAdapter {
  readonly identity = IDENTITY;

  recognizes(form: HTMLFormElement): boolean {
    return form.getAttribute(NATIVE_FORM_MARKER) === NATIVE_FORM_VALUE;
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

    if (element.tagName === "INPUT") {
      const type = ((element as { type?: string }).type ?? "").toLowerCase();
      if (type === "password" || type === "hidden") {
        return unsupported("password_or_hidden_field");
      }
    }

    const kind = editorKindOf(element);
    if (kind === undefined || kind === "contenteditable") {
      return unsupported("unsupported_editor");
    }
    if (element.isConnected !== true) {
      return { status: "invalidated", identity: IDENTITY, reason: "editor_detached" };
    }
    if (form.contains(element) !== true) {
      return { status: "invalidated", identity: IDENTITY, reason: "editor_detached" };
    }

    const editor = {
      kind,
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
