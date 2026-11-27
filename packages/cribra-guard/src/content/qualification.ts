/**
 * Qualification-only isolated-world test hook.
 *
 * The hook is installed only by the separate `.build/content-dist` package.
 * It is reachable only from the content script's isolated execution world and
 * never from the page. It returns source-free metadata, boolean verification
 * results, and replacement status only.
 *
 * This hook is not consent, browser identity, or submission authorization.
 */

import type {
  CaptureResult,
  CoverageLossReport,
  ReplacementResult,
} from "./contract.js";
import type { SubmissionInterceptor } from "./interceptor.js";
import {
  MAX_SNAPSHOTS,
  type SnapshotMetadata,
  type SnapshotStatus,
} from "./snapshot.js";

export interface ContentQualificationHook {
  readonly installed: () => boolean;
  readonly captures: () => readonly CaptureResult[];
  readonly losses: () => readonly CoverageLossReport[];
  readonly status: (snapshotId: string) => SnapshotStatus;
  readonly metadata: (snapshotId: string) => SnapshotMetadata | undefined;
  readonly verify: (snapshotId: string) => boolean;
  readonly sourceEquals: (snapshotId: string, candidate: string) => boolean;
  readonly replace: (snapshotId: string, text: string) => ReplacementResult;
  readonly focus: (snapshotId: string) => boolean;
  readonly release: (snapshotId: string) => boolean;
  readonly pending: () => readonly string[];
  readonly counts: () => Record<string, number>;
  readonly maxSnapshots: number;
  readonly dispose: () => void;
}

export function installQualificationHook(
  interceptor: SubmissionInterceptor,
): void {
  const hook: ContentQualificationHook = {
    installed: () => interceptor.isInstalled(),
    captures: () => interceptor.history(),
    losses: () => interceptor.losses(),
    status: (snapshotId) => interceptor.status(snapshotId),
    metadata: (snapshotId) => interceptor.metadata(snapshotId),
    verify: (snapshotId) => interceptor.verifySnapshot(snapshotId),
    sourceEquals: (snapshotId, candidate) =>
      interceptor.sourceEquals(snapshotId, candidate),
    replace: (snapshotId, text) =>
      interceptor.replaceSnapshot(snapshotId, text),
    focus: (snapshotId) => interceptor.focusSnapshot(snapshotId),
    release: (snapshotId) => interceptor.releaseSnapshot(snapshotId),
    pending: () => interceptor.pending(),
    counts: () => interceptor.counts(),
    maxSnapshots: MAX_SNAPSHOTS,
    dispose: () => interceptor.dispose(),
  };
  const globalObject = globalThis as unknown as {
    __cribraGuardQualification?: ContentQualificationHook;
  };
  globalObject.__cribraGuardQualification = hook;
}
