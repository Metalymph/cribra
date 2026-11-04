/** Internal normalization of generated session failures. */

import { CribraError } from "./errors.js";
import type { CribraErrorCode, ErrorDomain } from "./types.js";

const STREAM_CODES: readonly CribraErrorCode[] = [
  "stream_unsupported_ruleset",
  "stream_not_active",
  "stream_invalid_utf8",
  "stream_incomplete_utf8",
  "stream_candidate_limit",
  "stream_result_limit",
  "stream_storage_limit",
  "stream_storage_overflow",
  "stream_source_overflow",
  "stream_coordinate_overflow",
  "stream_invariant",
  "stream_unknown",
  "stream_invalid_limit",
];

const INCREMENTAL_CODES: readonly CribraErrorCode[] = [
  "incremental_not_active",
  "incremental_stale_revision",
  "incremental_invalid_range",
  "incremental_invalid_utf8_boundary",
  "incremental_revision_overflow",
  "incremental_document_length_overflow",
  "incremental_candidate_limit",
  "incremental_cache_limit",
  "incremental_result_limit",
  "incremental_transaction_storage_limit",
  "incremental_resource_overflow",
  "incremental_invariant",
  "incremental_unknown",
  "incremental_invalid_limit",
  "incremental_offset_out_of_range",
  "incremental_invalid_revision",
];

interface NativeSessionError {
  readonly code: number;
  readonly terminal?: boolean;
  free?: () => void;
}

function asNativeSessionError(value: unknown): NativeSessionError | undefined {
  if (value === null || typeof value !== "object") return undefined;
  try {
    const code = (value as { code?: unknown }).code;
    if (typeof code !== "number" || !Number.isInteger(code)) return undefined;
    const terminal = (value as { terminal?: unknown }).terminal;
    return {
      code,
      ...(typeof terminal === "boolean" ? { terminal } : {}),
    };
  } catch {
    return undefined;
  }
}

/** Releases one generated error object without letting free() mask the failure. */
function freeNativeSessionError(value: unknown): void {
  if (value === null || typeof value !== "object") return;
  const free = (value as { free?: unknown }).free;
  if (typeof free !== "function") return;
  try {
    (free as () => void).call(value);
  } catch {
    // A generated free() must not make an already-failed operation retryable.
  }
}

function normalizeNativeSessionError(
  error: unknown,
  codes: readonly CribraErrorCode[],
  unknownCode: CribraErrorCode,
  domain: ErrorDomain,
  withTerminal: boolean,
): CribraError {
  if (error instanceof CribraError) return error;
  const native = asNativeSessionError(error);
  freeNativeSessionError(error);

  const code =
    native !== undefined && native.code >= 0 && native.code < codes.length
      ? (codes[native.code] as CribraErrorCode)
      : unknownCode;
  const terminal = withTerminal ? (native?.terminal ?? false) : undefined;
  return new CribraError(code, {
    domain,
    ...(terminal === undefined ? {} : { terminal }),
  });
}

/** Normalizes a generated streaming failure. Its native object is freed once. */
export function normalizeStreamError(error: unknown): CribraError {
  return normalizeNativeSessionError(
    error,
    STREAM_CODES,
    "stream_unknown",
    "stream",
    false,
  );
}

/** Normalizes a generated incremental failure. Its native object is freed once. */
export function normalizeIncrementalError(error: unknown): CribraError {
  return normalizeNativeSessionError(
    error,
    INCREMENTAL_CODES,
    "incremental_unknown",
    "incremental",
    true,
  );
}
