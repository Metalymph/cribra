/**
 * Public entry point for the pure Guard policy and submission state machine.
 *
 * Future DOM controllers import from here. This module performs no I/O and
 * holds no global mutable state.
 *
 * Controller contract (future milestone):
 *
 * - Generate a unique `submissionId` for every intercepted submission, a
 *   stable `sessionId` for the browser session or document, and an opaque
 *   `snapshotId` for each exact source snapshot. Never reuse an identifier for a
 *   different operation.
 * - Provide trustworthy `tabId`, `frameId`, and adapter identity from the
 *   browser sender, not from page-controlled data. The pure machine can compare
 *   these values but cannot authenticate them.
 * - Compare exact source text before asserting a snapshot identity. The machine
 *   only compares opaque identifiers.
 * - Include `stage` on every scan result and the source `snapshotId` on every
 *   transform result so stale or unrelated async results cannot authorize a
 *   different operation.
 * - Treat `unsupported` coverage as unreviewable by policy: no transformation,
 *   no original approval, and no submit command.
 * - A confirmed snapshot mismatch permanently invalidates the transaction. A
 *   new submission and fresh scan are required.
 * - After a `submit` command is emitted, treat the external outcome as unknown
 *   if the transaction is later cancelled, navigated, timed out, disconnected,
 *   or otherwise invalidated. Authorization stays consumed, no retry occurs,
 *   and no second submit command is emitted. `completed` records adapter-level
 *   handoff only, not remote delivery.
 */

export {
  DEFAULT_TRANSFORMATION_PREFERENCE,
  GUARD_SEVERITIES,
  MAX_GUARD_IDENTIFIER_LENGTH,
  MAX_GUARD_ORIGIN_LENGTH,
  isValidFinalSnapshotVerification,
  isValidGuardIdentifier,
  isValidOriginalApprovalConfirmation,
  isValidTransactionIdentity,
  parseScanSummary,
  summarizeScan,
} from "./types.js";
export type {
  FinalSnapshotVerification,
  GuardAdapterIdentity,
  GuardSeverity,
  OriginalApprovalConfirmation,
  ScanCoverage,
  ScanSummary,
  TransactionIdentity,
  TransformationPreference,
} from "./types.js";

export { evaluatePolicy } from "./policy.js";
export type {
  CleanDecision,
  PolicyDecision,
  ReviewDecision,
  ReviewReason,
} from "./policy.js";

export {
  DEFAULT_GUARD_CONFIG,
  initialGuardState,
  transition,
} from "./machine.js";
export type {
  AdapterInvalidatedEvent,
  ApplyTransformationEvent,
  ApproveOriginalEvent,
  Authorization,
  AuthorizationKind,
  AuthorizedState,
  AwaitingReviewState,
  BeginEvent,
  BlockReason,
  BlockedState,
  CancelEvent,
  CancelReason,
  CancelledState,
  CompletedState,
  DocumentReplacedEvent,
  GuardCommand,
  GuardConfig,
  GuardEvent,
  GuardPhase,
  GuardState,
  IdleState,
  NavigatedEvent,
  OutcomeUnknownState,
  RevalidatingState,
  ReviewEditEvent,
  ScanCommand,
  ScanFailedEvent,
  ScanFailureReason,
  ScanStage,
  ScanSucceededEvent,
  ScannerDisconnectedEvent,
  SourceChangedEvent,
  SubmitCommand,
  SubmitFailedEvent,
  SubmitFailureReason,
  SubmitRequestedEvent,
  SubmitSucceededEvent,
  SubmissionConfirmation,
  SubmissionOutcomeUnknownReason,
  SubmittingState,
  TimeoutEvent,
  TransformCommand,
  TransformFailedEvent,
  TransformSucceededEvent,
  TransformingState,
  TransitionRejection,
  TransitionResult,
  ScanningState,
} from "./machine.js";
