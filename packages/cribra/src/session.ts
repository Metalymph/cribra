/** Internal validation and ownership helpers shared by both session kinds. */

import { CribraError } from "./errors.js";
import type {
  CribraErrorCode,
  ErrorDomain,
  IncrementalLimits,
  IncrementalOptions,
  StreamLimits,
  StreamOptions,
} from "./types.js";

/** The WASM target is 32-bit, so `usize::MAX` is 2^32 - 1. */
export const MAX_WASM_USIZE = 0xffff_ffff;
const MAX_U64 = (1n << 64n) - 1n;

export function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function invalidSessionArgument(domain: ErrorDomain): CribraError {
  return new CribraError("invalid_argument", {
    domain,
    ...(domain === "incremental" ? { terminal: false } : {}),
  });
}

function readLimit(
  source: Record<string, unknown>,
  field: string,
  code: CribraErrorCode,
  domain: ErrorDomain,
): number | undefined {
  const value = source[field];
  if (value === undefined) return undefined;
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < 0 ||
    value > MAX_WASM_USIZE
  ) {
    throw new CribraError(code, {
      domain,
      ...(domain === "incremental" ? { terminal: false } : {}),
    });
  }
  return value;
}

function limitsRecord(
  options: unknown,
  domain: ErrorDomain,
): Record<string, unknown> | undefined {
  if (options === undefined) return undefined;
  if (!isRecord(options)) throw invalidSessionArgument(domain);
  const limits = options.limits;
  if (limits === undefined) return undefined;
  if (!isRecord(limits)) throw invalidSessionArgument(domain);
  return limits;
}

/** Returns undefined when no limits were supplied, otherwise a fresh native-shaped object. */
export function normalizeStreamLimits(
  options: StreamOptions | undefined,
): StreamLimits | undefined {
  const limits = limitsRecord(options, "stream");
  if (limits === undefined) return undefined;

  const maxCandidates = readLimit(
    limits,
    "maxCandidates",
    "stream_invalid_limit",
    "stream",
  );
  const maxStreamStorageBytes = readLimit(
    limits,
    "maxStreamStorageBytes",
    "stream_invalid_limit",
    "stream",
  );
  const maxRetainedResults = readLimit(
    limits,
    "maxRetainedResults",
    "stream_invalid_limit",
    "stream",
  );

  return {
    ...(maxCandidates === undefined ? {} : { maxCandidates }),
    ...(maxStreamStorageBytes === undefined ? {} : { maxStreamStorageBytes }),
    ...(maxRetainedResults === undefined ? {} : { maxRetainedResults }),
  };
}

/** Returns undefined when no limits were supplied, otherwise a fresh native-shaped object. */
export function normalizeIncrementalLimits(
  options: IncrementalOptions | undefined,
): IncrementalLimits | undefined {
  const limits = limitsRecord(options, "incremental");
  if (limits === undefined) return undefined;

  const maxCandidates = readLimit(
    limits,
    "maxCandidates",
    "incremental_invalid_limit",
    "incremental",
  );
  const maxRetainedCacheRecords = readLimit(
    limits,
    "maxRetainedCacheRecords",
    "incremental_invalid_limit",
    "incremental",
  );
  const maxMaterializedResults = readLimit(
    limits,
    "maxMaterializedResults",
    "incremental_invalid_limit",
    "incremental",
  );
  const maxTransactionStorageBytes = readLimit(
    limits,
    "maxTransactionStorageBytes",
    "incremental_invalid_limit",
    "incremental",
  );

  return {
    ...(maxCandidates === undefined ? {} : { maxCandidates }),
    ...(maxRetainedCacheRecords === undefined
      ? {}
      : { maxRetainedCacheRecords }),
    ...(maxMaterializedResults === undefined
      ? {}
      : { maxMaterializedResults }),
    ...(maxTransactionStorageBytes === undefined
      ? {}
      : { maxTransactionStorageBytes }),
  };
}

export function isValidOffset(value: unknown): value is number {
  return (
    typeof value === "number" &&
    Number.isSafeInteger(value) &&
    value >= 0 &&
    value <= MAX_WASM_USIZE
  );
}

export function isValidRevision(value: unknown): value is bigint {
  return typeof value === "bigint" && value >= 0n && value <= MAX_U64;
}

export function safeFree(handle: { free(): void } | undefined): void {
  if (handle === undefined) return;
  try {
    handle.free();
  } catch {
    // A generated free() must never make disposal retryable or mask failure.
  }
}
