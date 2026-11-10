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

export interface Spec extends TurboModule {
  getAbiVersion(): AbiVersion;
  createDefaultScanner(): Promise<number>;
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
