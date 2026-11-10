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

export interface Spec extends TurboModule {
  getAbiVersion(): AbiVersion;
  createDefaultScanner(): Promise<number>;
  scan(
    scannerId: number,
    sourceId: string,
    text: string,
  ): Promise<ScanReportDto>;
  disposeScanner(scannerId: number): Promise<void>;
  getLiveScannerCount(): number;
}

export default TurboModuleRegistry.getEnforcing<Spec>('CribraReactNative');
