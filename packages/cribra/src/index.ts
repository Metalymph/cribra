/** Default/browser public entry. Importing this module performs no WASM work. */

import { createRuntime } from "./core.js";
import { browserLoader } from "./loaders/browser.js";
import { createScanner as createScannerImpl } from "./scanner.js";
import type { Scanner, ScannerOptions } from "./types.js";

const runtime = createRuntime(
  browserLoader,
  new URL("./wasm/cribra_bg.wasm", import.meta.url),
);

export const init = runtime.init;
export const isInitialized = runtime.isInitialized;
export const getCapabilities = runtime.getCapabilities;
export const getVersion = runtime.getVersion;

/**
 * Requires a successfully initialized backend.
 * Throws CribraError("not_initialized") otherwise.
 * Synchronous: rule validation and compilation complete before it returns.
 */
export function createScanner(options?: ScannerOptions): Scanner {
  return createScannerImpl(runtime, options);
}

export { CribraError } from "./errors.js";
export { byteSpanToTextRange } from "./offset.js";
export { createShareBundle } from "./share.js";

export type {
  Backend,
  BatchEntry,
  BatchResult,
  ByteSpan,
  Candidate,
  CandidateEvidence,
  CandidateKind,
  Capabilities,
  Confidence,
  CribraErrorCode,
  DetectionMode,
  ErrorDomain,
  Explanation,
  Finding,
  IncrementalLimits,
  IncrementalOptions,
  IncrementalSession,
  IncrementalSnapshot,
  InitOptions,
  LiteralRuleSpec,
  PatternRuleSpec,
  PrefixRuleSpec,
  Profile,
  PseudonymizeRequest,
  RedactRequest,
  Remediation,
  RuleKind,
  RuleMetadata,
  RuleSpec,
  ScanOptions,
  ScanResult,
  ScanSource,
  ScanSummary,
  Scanner,
  ScannerOptions,
  Severity,
  ShareBundle,
  ShareBundleEntry,
  ShareBundleOptions,
  ShareManifest,
  ShareMode,
  ShareModeKind,
  StreamLimits,
  StreamOptions,
  StreamSession,
  SuffixRuleSpec,
  SynthesizeRequest,
  TemplateRequest,
  TextRange,
  TransformRequest,
  TransformResult,
  VersionInfo,
} from "./types.js";
