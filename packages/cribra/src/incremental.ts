/** Owned editable UTF-8 document with exact bigint revisions. */

import { CribraError } from "./errors.js";
import { normalizeIncrementalError } from "./native-errors.js";
import { projectSessionScanResult } from "./result.js";
import { isValidOffset, isValidRevision, safeFree } from "./session.js";
import type {
  IncrementalSession,
  IncrementalSnapshot,
} from "./types.js";
import type {
  IncrementalSession as NativeIncrementalSession,
  IncrementalSnapshot as NativeIncrementalSnapshot,
} from "./wasm/cribra.js";

function disposedError(): CribraError {
  return new CribraError("disposed", { domain: "incremental" });
}

export function createIncrementalSession(
  native: NativeIncrementalSession,
): IncrementalSession {
  let disposed = false;
  let active = native.isActive();

  function requireNotDisposed(): void {
    if (disposed) throw disposedError();
  }

  function invariant(): CribraError {
    return new CribraError("incremental_invariant", {
      domain: "incremental",
      terminal: !active,
    });
  }

  return Object.freeze({
    get disposed() {
      return disposed;
    },
    get isActive() {
      return active;
    },
    applyEdit(
      expectedRevision: bigint,
      start: number,
      end: number,
      replacement: string,
    ): bigint {
      requireNotDisposed();
      if (typeof replacement !== "string") {
        throw new CribraError("invalid_argument", {
          domain: "incremental",
          terminal: !active,
        });
      }
      if (!isValidRevision(expectedRevision)) {
        throw new CribraError("incremental_invalid_revision", {
          domain: "incremental",
          terminal: !active,
        });
      }
      if (!isValidOffset(start)) {
        throw new CribraError("incremental_offset_out_of_range", {
          domain: "incremental",
          terminal: !active,
        });
      }
      if (!isValidOffset(end)) {
        throw new CribraError("incremental_offset_out_of_range", {
          domain: "incremental",
          terminal: !active,
        });
      }

      try {
        const revision = native.applyEdit(
          expectedRevision,
          start,
          end,
          replacement,
        );
        active = true;
        return revision;
      } catch (error) {
        const normalized = normalizeIncrementalError(error);
        try {
          active = native.isActive();
        } catch {
          if (normalized.terminal === true) active = false;
        }
        throw normalized;
      }
    },
    snapshot(): IncrementalSnapshot {
      requireNotDisposed();

      let snapshot: NativeIncrementalSnapshot;
      try {
        snapshot = native.snapshot();
      } catch {
        throw new CribraError("incremental_unknown", {
          domain: "incremental",
          terminal: !active,
        });
      }

      try {
        const revision = snapshot.revision;
        if (typeof revision !== "bigint") throw invariant();

        let nativeResult: import("./wasm/cribra.js").ScanResult;
        try {
          nativeResult = snapshot.result;
        } catch {
          throw invariant();
        }

        try {
          try {
            return Object.freeze({
              revision,
              result: projectSessionScanResult(nativeResult),
            });
          } catch {
            throw invariant();
          }
        } finally {
          safeFree(nativeResult);
        }
      } finally {
        safeFree(snapshot);
      }
    },
    dispose(): void {
      if (disposed) return;
      disposed = true;
      active = false;
      safeFree(native);
    },
  });
}
