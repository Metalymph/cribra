/**
 * Public surface for the qualification-only content layer.
 *
 * Trust boundary:
 * - The page DOM is untrusted. Fixture markers, page attributes, page events,
 *   and page messages are never proof of consent, browser identity, or
 *   submission authorization.
 * - Capture results are source-free diagnostics. A future controller must
 *   authenticate the extension sender, derive tab/frame/adapter identity from
 *   trusted browser APIs, and bind authorization to exact snapshots.
 * - `preventDefault` does not stop direct fetch/XHR, `form.submit()`,
 *   non-cancellable events, earlier window capture handlers, autosave, or
 *   unsupported editors. Those limits are documented and tested.
 * - DOM observation cannot detect every transient or silent mutation.
 */

export type {
  AdapterCoverage,
  AdapterIdentity,
  CaptureReason,
  CaptureResult,
  CapturedResult,
  CoverageFailureReason,
  CoverageLossReason,
  CoverageLossReport,
  DegradedCaptureResult,
  DegradedCoverage,
  DuplicateResult,
  EditorKind,
  EditorRef,
  InvalidatedCaptureResult,
  InvalidatedCoverage,
  NotCancellableResult,
  PageAdapter,
  ReplacementFailure,
  ReplacementFailureReason,
  ReplacementResult,
  ReplacementSuccess,
  SubmitterKind,
  SupportedCoverage,
  UnsupportedCoverage,
  UnsupportedIgnoredResult,
} from "./contract.js";

export {
  EDITOR_MARKER,
  EDITOR_VALUE,
  NATIVE_FORM_ADAPTER_ID,
  NATIVE_FORM_MARKER,
  NATIVE_FORM_VALUE,
  NativeFormAdapter,
} from "./native-form-adapter.js";
export {
  CONTENTEDITABLE_ADAPTER_ID,
  CONTENTEDITABLE_MARKER,
  CONTENTEDITABLE_VALUE,
  ContentEditableAdapter,
} from "./contenteditable-adapter.js";
export {
  AdapterRegistry,
  REGISTRY_IDENTITY,
  type ProtectedRecord,
  type RegistryOptions,
} from "./registry.js";
export {
  MAX_SNAPSHOTS,
  SnapshotStore,
  type SnapshotCounts,
  type SnapshotInvalidationReason,
  type SnapshotMetadata,
  type SnapshotStatus,
} from "./snapshot.js";
export {
  SubmissionInterceptor,
  type InterceptorOptions,
} from "./interceptor.js";
export {
  editorKindOf,
  focusEditor,
  formFromEventTarget,
  isContentEditableElement,
  isEditorCurrent,
  isEditorWritable,
  isFormElement,
  isImageSubmitter,
  isSubmitButton,
  isTextAreaControl,
  isTextInputControl,
  readEditor,
  replaceEditor,
  submitterKindOf,
  writabilityReason,
} from "./editors.js";
export {
  newSnapshotId,
  objectToken,
} from "./tokens.js";
export type { ContentQualificationHook } from "./qualification.js";
