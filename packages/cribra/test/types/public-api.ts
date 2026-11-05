/* Compile-time public surface check. Never emitted. */

// @ts-expect-error Native generated classes must not be exported.
import type { ScanEngine } from "cribra-typescript";
// @ts-expect-error Flow-only generated classes must not be exported.
import type { ShareBundleBuilder } from "cribra-typescript";
// @ts-expect-error Native generated error classes must not be exported.
import type { TransformError } from "cribra-typescript";

import {
  CribraError,
  byteSpanToTextRange,
  createScanner,
  createShareBundle,
  getCapabilities,
  getVersion,
  init,
  isInitialized,
} from "cribra-typescript";
import type {
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
} from "cribra-typescript";

const literal: LiteralRuleSpec = {
  kind: "literal",
  id: "t.literal",
  severity: "high",
  value: "x",
  remediation: "rotate_credential",
};
const prefix: PrefixRuleSpec = {
  kind: "prefix",
  id: "t.prefix",
  severity: "medium",
  value: "pre_",
};
const suffix: SuffixRuleSpec = {
  kind: "suffix",
  id: "t.suffix",
  severity: "low",
  value: "_end",
  remediation: null,
};
const pattern: PatternRuleSpec = {
  kind: "pattern",
  id: "t.pattern",
  severity: "critical",
  pattern: "token-[0-9]{4}",
};
const rules: readonly RuleSpec[] = [literal, prefix, suffix, pattern];
const options: ScannerOptions = { builtins: "none", rules };
const initOptions: InitOptions = { wasm: new Uint8Array() };

const scanner: Scanner = createScanner(options);
const result: ScanResult = scanner.scan("token-1234", { includeExplanations: true });
const batch: BatchResult = scanner.scanBatch([
  { id: "", source: "token-1234" } satisfies ScanSource,
]);
const entry: BatchEntry = batch.entries[0]!;
const summary: ScanSummary = batch.summary;
const finding: Finding = result.findings[0]!;
const candidate: Candidate = result.candidates[0]!;
const span: ByteSpan = finding.span;
const range: TextRange = byteSpanToTextRange("token-1234", span);
const explanation: Explanation = finding.explanation ?? candidate.explanation;
const metadata: RuleMetadata = scanner.rules[0]!;
const capabilities: Capabilities = getCapabilities();
const version: VersionInfo = getVersion();

const severity: Severity = finding.severity;
const confidence: Confidence = finding.confidence;
const detectionMode: DetectionMode =
  explanation.kind === "classified" ? explanation.detectionMode : "matcher_only";
const ruleKind: RuleKind = metadata.kind;
const candidateKind: CandidateKind = candidate.kind;
const evidence: CandidateEvidence = candidate.evidence;
const remediation: Remediation | null = finding.remediation;
const profile: Profile = capabilities.profile;
const domain: ErrorDomain = new CribraError("invalid_argument").domain;
const code: CribraErrorCode = new CribraError("invalid_argument").code;
const scanOptions: ScanOptions = { includeExplanations: false };
const initialized: Promise<void> = init(initOptions);
const ready: boolean = isInitialized();

const streamLimits: StreamLimits = {
  maxCandidates: 1,
  maxStreamStorageBytes: 1024,
  maxRetainedResults: 1,
};
const streamOptions: StreamOptions = { limits: streamLimits };
const stream: StreamSession = scanner.stream(streamOptions);
stream.push(new Uint8Array());
const streamed: ScanResult = stream.finish();
stream.dispose();

const incrementalLimits: IncrementalLimits = {
  maxCandidates: 1,
  maxRetainedCacheRecords: 1,
  maxMaterializedResults: 1,
  maxTransactionStorageBytes: 1024,
};
const incrementalOptions: IncrementalOptions = {
  limits: incrementalLimits,
};
const incremental: IncrementalSession = scanner.incremental(
  "token-1234",
  incrementalOptions,
);
const revision: bigint = incremental.applyEdit(0n, 0, 0, "");
const incrementalSnapshot: IncrementalSnapshot = incremental.snapshot();
const snapshotRevision: bigint = incrementalSnapshot.revision;
const snapshotResult: ScanResult = incrementalSnapshot.result;
const incrementalActive: boolean = incremental.isActive;
const incrementalDisposed: boolean = incremental.disposed;
incremental.dispose();

const terminal: boolean | undefined = new CribraError(
  "incremental_stale_revision",
).terminal;

const redactRequest: RedactRequest = { kind: "redact", replacement: "<x>" };
const templateRequest: TemplateRequest = {
  kind: "template",
  namespace: "SAFE",
  numbered: true,
};
const pseudonymizeRequest: PseudonymizeRequest = {
  kind: "pseudonymize",
  key: new Uint8Array(32),
  prefix: "p_",
  digestBytes: 16,
};
const synthesizeRequest: SynthesizeRequest = {
  kind: "synthesize",
  key: new Uint8Array(32),
  marker: "synthetic",
};
const transformRequests: readonly TransformRequest[] = [
  redactRequest,
  templateRequest,
  pseudonymizeRequest,
  synthesizeRequest,
];
const transformed: TransformResult = scanner.transform(
  "token-1234",
  transformRequests[0]!,
);
const transformedScan: ScanResult = transformed.scan;
const transformedOutput: string = transformed.output;

const redactMode: ShareMode = { kind: "redact" };
const templateMode: ShareMode = { kind: "template" };
const pseudonymizeMode: ShareMode = {
  kind: "pseudonymize",
  key: new Uint8Array(32),
};
const synthesizeMode: ShareMode = {
  kind: "synthesize",
  key: new Uint8Array(32),
};
const bundleOptions: ShareBundleOptions = {
  mode: redactMode,
  generatedAt: new Date(0),
};
const shareBundle: ShareBundle = createShareBundle(
  scanner,
  [{ id: "x", source: "token-1234" }],
  bundleOptions,
);
const bundleEntries: readonly ShareBundleEntry[] = shareBundle.entries;
const bundleManifest: ShareManifest = shareBundle.manifest;
const bundleMode: ShareModeKind = bundleManifest.mode;
const bundleSeconds: number = bundleManifest.generatedAtSeconds;
const bundleNanoseconds: number = bundleManifest.generatedAtNanoseconds;
const bundleSummary: ScanSummary = shareBundle.summary;
const manifestSummary: ScanSummary = bundleManifest.summary;

// @ts-expect-error Share mode requires a discriminated kind.
const invalidMode: ShareMode = {};
// @ts-expect-error Transformation keys are Uint8Array only.
const invalidKey: PseudonymizeRequest = { kind: "pseudonymize", key: "x" };

void [
  entry,
  summary,
  range,
  severity,
  confidence,
  detectionMode,
  ruleKind,
  candidateKind,
  evidence,
  remediation,
  profile,
  domain,
  code,
  scanOptions,
  initialized,
  ready,
  streamed,
  revision,
  snapshotRevision,
  snapshotResult,
  incrementalActive,
  incrementalDisposed,
  terminal,
  templateMode,
  pseudonymizeMode,
  synthesizeMode,
  transformedScan,
  transformedOutput,
  bundleEntries,
  bundleMode,
  bundleSeconds,
  bundleNanoseconds,
  bundleSummary,
  manifestSummary,
  invalidMode,
  invalidKey,
];
