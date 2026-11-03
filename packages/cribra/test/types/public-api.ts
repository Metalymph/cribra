/* Compile-time public surface check. Never emitted. */

// @ts-expect-error Native generated classes must not be exported.
import type { ScanEngine } from "cribra-typescript";

import {
  CribraError,
  byteSpanToTextRange,
  createScanner,
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

// @ts-expect-error Streaming is deferred in W3.2B-2.
scanner.stream();

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
];
