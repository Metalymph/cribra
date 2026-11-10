import { TurboModuleRegistry, type TurboModule } from 'react-native';

export type AbiVersion = {
  major: number;
  minor: number;
  patch: number;
};

export type FindingDto = {
  ruleId: string;
  start: number;
  end: number;
  line: number;
  column: number;
  severity: string;
  confidence: string;
  remediation: string;
  explanationKind: string;
  explanationValue: string;
};

export type CandidateDto = {
  kind: string;
  start: number;
  end: number;
  line: number;
  column: number;
  evidence: string;
  explanationKind: string;
  explanationValue: string;
};

export type ScanReportDto = {
  sourceId: string;
  sourceBytes: number;
  findings: FindingDto[];
  candidateCount: number;
  needsReview: boolean;
  candidates: CandidateDto[];
};

export type ScanSourceDto = {
  id: string;
  source: string;
};

export type ScanSummaryDto = {
  scannedSources: number;
  scannedBytes: number;
  reportsWithFindings: number;
  reportsWithoutFindings: number;
  totalFindings: number;
  totalCandidates: number;
  reportsWithCandidates: number;
  hasCandidates: boolean;
  critical: number;
  high: number;
  medium: number;
  low: number;
  info: number;
  isClean: boolean;
  hasCritical: boolean;
};

export type ScanBatchDto = {
  entries: ScanReportDto[];
  summary: ScanSummaryDto;
};

export type RuleSpecDto = {
  kind: string;
  id: string;
  value: string;
  severity: string;
  remediation: string;
};

export type RuleMetadataDto = {
  ruleId: string;
  kind: string;
  detectionMode: string;
  severity: string;
  remediation: string;
};

/**
 * Result of one configured scanner construction. A positive `scannerId` means
 * success; a zero `scannerId` carries a source-free failure category plus the
 * structured Core build metadata when the failure was a rule/scanner build.
 */
export type ScannerCreationDto = {
  scannerId: number;
  rules: RuleMetadataDto[];
  failure: string;
  buildCode: number;
  ruleId: string;
  detail: string;
};

export interface Spec extends TurboModule {
  getAbiVersion(): AbiVersion;
  createDefaultScanner(): Promise<number>;
  /**
   * One native construction operation. Configuration is decoded and copied on
   * the JS thread, then one serial-worker job validates Core catalogs/rules,
   * builds the scanner, and projects detached rule metadata before the scanner
   * is registered.
   */
  createScanner(
    builtins: string,
    financialBuiltins: boolean,
    personalBuiltins: boolean,
    rules: RuleSpecDto[],
  ): Promise<ScannerCreationDto>;
  scan(
    scannerId: number,
    sourceId: string,
    text: string,
  ): Promise<ScanReportDto>;
  /**
   * One native batch operation. The native side copies every input on the JS
   * thread, posts one serial-worker job, runs `cribra_scanner_scan` once per
   * source in order, and resolves one detached DTO batch.
   */
  scanBatch(
    scannerId: number,
    sources: ScanSourceDto[],
  ): Promise<ScanBatchDto>;
  disposeScanner(scannerId: number): Promise<void>;
  getLiveScannerCount(): number;
}

export default TurboModuleRegistry.getEnforcing<Spec>('CribraReactNative');
