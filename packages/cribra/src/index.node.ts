/** Node ESM entry. Only this graph may import Node built-ins. */

import { createRuntime } from "./core.js";
import { nodeLoader } from "./loaders/node.js";
import { createScanner as createScannerImpl } from "./scanner.js";
import type { Scanner, ScannerOptions } from "./types.js";

const runtime = createRuntime(
  nodeLoader,
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
  InitOptions,
  LiteralRuleSpec,
  PatternRuleSpec,
  PrefixRuleSpec,
  Profile,
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
  SuffixRuleSpec,
  TextRange,
  VersionInfo,
} from "./types.js";
