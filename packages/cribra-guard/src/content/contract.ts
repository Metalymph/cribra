/**
 * Typed content-script adapter contract.
 *
 * The page DOM is untrusted. These types describe only local adapter coverage,
 * exact source snapshots, and controlled editor operations. Capture results are
 * deliberately source-free and are never proof of user consent, browser
 * identity, or submission authorization.
 *
 * A future trusted controller must authenticate extension message senders,
 * verify origin/tab/frame identity, and bind authorization to exact snapshots.
 */

export interface AdapterIdentity {
  readonly id: string;
  readonly version: string;
}

export type EditorKind = "textarea" | "text-input" | "contenteditable";

export type CoverageFailureReason =
  | "unrecognized_form"
  | "no_eligible_editor"
  | "multiple_eligible_editors"
  | "password_or_hidden_field"
  | "unsupported_editor"
  | "unsupported_submitter"
  | "editor_detached"
  | "form_detached"
  | "editor_replaced"
  | "adapter_changed"
  | "adapter_invalidated"
  | "adapter_disposed"
  | "editor_readonly"
  | "editor_disabled"
  | "editor_not_editable"
  | "oversized_source";

/** A live editor reference. Never serialized or sent across a boundary. */
export interface EditorRef {
  readonly kind: EditorKind;
  readonly element: HTMLElement;
  readonly form: HTMLFormElement;
  /** Opaque per-element token; stable for the element instance only. */
  readonly token: string;
}

interface CoverageBase {
  readonly identity: AdapterIdentity;
}

export interface SupportedCoverage extends CoverageBase {
  readonly status: "supported";
  readonly adapter: PageAdapter;
  readonly editor: EditorRef;
  readonly formToken: string;
}

export interface UnsupportedCoverage extends CoverageBase {
  readonly status: "unsupported";
  readonly reason: CoverageFailureReason;
}

export interface InvalidatedCoverage extends CoverageBase {
  readonly status: "invalidated";
  readonly reason: CoverageFailureReason;
  /** The underlying detection failure when the reason is generic. */
  readonly cause?: CoverageFailureReason;
}

export interface DegradedCoverage extends CoverageBase {
  readonly status: "degraded";
  readonly reason: CoverageFailureReason;
}

export type AdapterCoverage =
  | SupportedCoverage
  | UnsupportedCoverage
  | InvalidatedCoverage
  | DegradedCoverage;

export type ReplacementFailureReason =
  | "snapshot_unavailable"
  | "editor_detached"
  | "form_detached"
  | "read_only"
  | "editor_disabled"
  | "unsupported_editor"
  | "write_failed"
  | "read_back_mismatch";

export interface ReplacementSuccess {
  readonly status: "replaced";
  readonly readBackVerified: true;
}

export interface ReplacementFailure {
  readonly status: "failed";
  readonly reason: ReplacementFailureReason;
}

export type ReplacementResult = ReplacementSuccess | ReplacementFailure;

/**
 * A page adapter owns exactly one explicitly recognized submission shape.
 *
 * Detection, reading, and interception are synchronous. The adapter must not
 * claim coverage for arbitrary forms, fields, editors, or submission channels.
 */
export interface PageAdapter {
  readonly identity: AdapterIdentity;
  /** True only for forms this adapter explicitly recognizes. */
  recognizes(form: HTMLFormElement): boolean;
  /** Synchronous coverage detection for the current DOM. */
  detect(form: HTMLFormElement): AdapterCoverage;
  /** Whether this submitter is a supported submission intent. */
  supportsSubmitter(
    coverage: SupportedCoverage,
    submitter: HTMLElement | null,
  ): boolean;
  /** Exact logical source text. No normalization. */
  read(coverage: SupportedCoverage): string;
  /** Deterministic replacement with internal read-back verification. */
  replace(coverage: SupportedCoverage, text: string): ReplacementResult;
  /** Best-effort focus; never submits. */
  focus(coverage: SupportedCoverage): void;
}

export type SubmitterKind = "submit" | "image" | "implicit";

export type CaptureReason = CoverageFailureReason | "not_cancellable";

export interface CapturedResult {
  readonly kind: "captured";
  readonly identity: AdapterIdentity;
  readonly snapshotId: string;
  readonly formToken: string;
  readonly submitterToken: string | null;
  readonly submitterKind: SubmitterKind;
  readonly editorKind: EditorKind;
  readonly blocked: true;
  readonly cancelled: true;
  readonly at: number;
}

export interface DuplicateResult {
  readonly kind: "duplicate";
  readonly identity: AdapterIdentity;
  readonly snapshotId: string;
  readonly formToken: string;
  readonly blocked: true;
  readonly cancelled: true;
  readonly at: number;
}

export interface InvalidatedCaptureResult {
  readonly kind: "invalidated";
  readonly identity: AdapterIdentity;
  readonly formToken: string;
  readonly reason: CoverageFailureReason;
  readonly cause?: CoverageFailureReason;
  readonly blocked: true;
  readonly cancelled: true;
  readonly at: number;
}

export interface DegradedCaptureResult {
  readonly kind: "degraded";
  readonly identity: AdapterIdentity;
  readonly formToken: string;
  readonly reason: CoverageFailureReason;
  readonly blocked: true;
  readonly cancelled: true;
  readonly at: number;
}

export interface NotCancellableResult {
  readonly kind: "not_cancellable";
  readonly identity: AdapterIdentity;
  readonly formToken: string;
  readonly reason: CaptureReason;
  readonly blocked: false;
  readonly cancelled: false;
  readonly at: number;
}

export interface UnsupportedIgnoredResult {
  readonly kind: "unsupported_ignored";
  readonly identity: AdapterIdentity;
  readonly reason: CoverageFailureReason;
  readonly blocked: false;
  readonly cancelled: false;
  readonly at: number;
}

export type CaptureResult =
  | CapturedResult
  | DuplicateResult
  | InvalidatedCaptureResult
  | DegradedCaptureResult
  | NotCancellableResult
  | UnsupportedIgnoredResult;

export type CoverageLossReason =
  | "form_replaced"
  | "form_detached"
  | "editor_replaced"
  | "editor_detached"
  | "document_replaced"
  | "adapter_invalidated"
  | "not_cancellable";

/**
 * Evidence that a previously recognized path can no longer be fully protected.
 * A loss report is diagnostic only: it never claims interception or protection.
 */
export interface CoverageLossReport {
  readonly kind: "coverage_lost";
  readonly identity: AdapterIdentity;
  readonly formToken: string;
  readonly reason: CoverageLossReason;
  readonly at: number;
}
