/** Owned synchronous byte stream with a scanner-independent lifetime. */

import { CribraError } from "./errors.js";
import { normalizeStreamError } from "./native-errors.js";
import { projectSessionScanResult } from "./result.js";
import { invalidSessionArgument, safeFree } from "./session.js";
import type { ScanResult, StreamSession } from "./types.js";
import type { StreamSession as NativeStreamSession } from "./wasm/cribra.js";

type StreamStatus = "active" | "finished" | "failed" | "disposed";

export function createStreamSession(native: NativeStreamSession): StreamSession {
  let status: StreamStatus = "active";

  function requireActive(): void {
    if (status === "disposed") {
      throw new CribraError("disposed", { domain: "stream" });
    }
    if (status !== "active") {
      throw new CribraError("stream_not_active", { domain: "stream" });
    }
  }

  function release(next: "finished" | "failed"): void {
    if (status !== "active") return;
    status = next;
    safeFree(native);
  }

  return Object.freeze({
    get disposed() {
      return status !== "active";
    },
    push(chunk: Uint8Array): void {
      requireActive();
      if (!(chunk instanceof Uint8Array)) {
        throw invalidSessionArgument("stream");
      }
      try {
        // The generated binding copies this chunk into Core; it is never retained.
        native.push(chunk);
      } catch (error) {
        const normalized = normalizeStreamError(error);
        release("failed");
        throw normalized;
      }
    },
    finish(): ScanResult {
      requireActive();

      let nativeResult: import("./wasm/cribra.js").ScanResult;
      try {
        nativeResult = native.finish();
      } catch (error) {
        const normalized = normalizeStreamError(error);
        release("failed");
        throw normalized;
      }

      try {
        try {
          return projectSessionScanResult(nativeResult);
        } catch {
          throw new CribraError("stream_invariant", { domain: "stream" });
        }
      } finally {
        safeFree(nativeResult);
        release("finished");
      }
    },
    dispose(): void {
      if (status !== "active") return;
      status = "disposed";
      safeFree(native);
    },
  });
}
