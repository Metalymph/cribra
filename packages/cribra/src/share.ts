/** Core-backed Share Bundle creation with detached DTO projection. */

import { CribraError } from "./errors.js";
import { shareModeFromNative } from "./enums.js";
import {
  normalizeShareBundleError,
  normalizeTransformError,
} from "./native-errors.js";
import { projectSummary } from "./result.js";
import { scannerInternals } from "./scanner-registry.js";
import { isRecord, MAX_WASM_USIZE, safeFree } from "./session.js";
import type {
  ScanSource,
  Scanner,
  ShareBundle,
  ShareBundleEntry,
  ShareBundleOptions,
  ShareManifest,
  ShareMode,
} from "./types.js";
import type {
  ScanEngine,
  ScanSummaryView as NativeScanSummaryView,
} from "./wasm/cribra.js";

/** Generated Share Bundle classes exist only in Flow; keep them structural. */
interface NativeShareBundleEntry {
  readonly id: string;
  readonly content: string;
  free(): void;
}

interface NativeShareManifest {
  readonly mode: number;
  readonly generatedAtSeconds: bigint;
  readonly generatedAtNanoseconds: number;
  summary(): NativeScanSummaryView;
  free(): void;
}

interface NativeShareBundle {
  readonly length: number;
  isEmpty(): boolean;
  entryAt(index: number): NativeShareBundleEntry;
  manifest(): NativeShareManifest;
  summary(): NativeScanSummaryView;
  free(): void;
}

interface NativeShareBundleBuilder {
  modeRedact(): void;
  modeTemplate(): void;
  modePseudonymize(
    key: Uint8Array,
    prefix?: string | null,
    digestBytes?: number | null,
  ): void;
  modeSynthesize(key: Uint8Array, marker?: string | null): void;
  scanAndBuild(engine: ScanEngine, sources: ScanSource[]): NativeShareBundle;
  scanAndBuildWithGeneratedAt(
    engine: ScanEngine,
    sources: ScanSource[],
    generatedAtMillis: number,
  ): NativeShareBundle;
  free(): void;
}

interface NativeShareModule {
  readonly ShareBundleBuilder: new () => NativeShareBundleBuilder;
}

const KEY_BYTES = 32;
const MAX_SAFE_SECONDS = BigInt(Number.MAX_SAFE_INTEGER);
const NANOS_PER_SECOND = 1_000_000_000;

function invalidArgument(): CribraError {
  return new CribraError("invalid_argument", { domain: "share" });
}

function invalidKeyLength(): CribraError {
  return new CribraError("invalid_key_length", { domain: "share" });
}

function requireKey(value: unknown): Uint8Array {
  if (!(value instanceof Uint8Array) || value.byteLength !== KEY_BYTES) {
    throw invalidKeyLength();
  }
  return value;
}

function optionalString(
  source: Record<string, unknown>,
  field: string,
): string | undefined {
  const value = source[field];
  if (value === undefined) return undefined;
  if (typeof value !== "string") throw invalidArgument();
  return value;
}

function optionalDigestBytes(
  source: Record<string, unknown>,
): number | undefined {
  const value = source.digestBytes;
  if (value === undefined) return undefined;
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < 0 ||
    value > MAX_WASM_USIZE
  ) {
    throw invalidArgument();
  }
  return value;
}

function normalizeMode(value: unknown): ShareMode {
  if (!isRecord(value)) throw invalidArgument();
  switch (value.kind) {
    case "redact":
      return { kind: "redact" };
    case "template":
      return { kind: "template" };
    case "pseudonymize": {
      const key = requireKey(value.key);
      const prefix = optionalString(value, "prefix");
      const digestBytes = optionalDigestBytes(value);
      return {
        kind: "pseudonymize",
        key,
        ...(prefix === undefined ? {} : { prefix }),
        ...(digestBytes === undefined ? {} : { digestBytes }),
      };
    }
    case "synthesize": {
      const key = requireKey(value.key);
      const marker = optionalString(value, "marker");
      return {
        kind: "synthesize",
        key,
        ...(marker === undefined ? {} : { marker }),
      };
    }
    default:
      throw invalidArgument();
  }
}

function normalizeSources(sources: unknown): ScanSource[] {
  if (!Array.isArray(sources)) throw invalidArgument();
  return sources.map((entry) => {
    if (!isRecord(entry)) throw invalidArgument();
    if (
      typeof entry.id !== "string" ||
      typeof entry.source !== "string"
    ) {
      throw invalidArgument();
    }
    return { id: entry.id, source: entry.source };
  });
}

interface NormalizedOptions {
  readonly mode: ShareMode;
  readonly generatedAtMillis?: number;
}

function normalizeOptions(options: unknown): NormalizedOptions {
  if (!isRecord(options)) throw invalidArgument();
  const mode = normalizeMode(options.mode);
  const generatedAt = options.generatedAt;
  if (generatedAt === undefined) return { mode };
  if (!(generatedAt instanceof Date)) throw invalidArgument();
  const millis = generatedAt.getTime();
  if (
    !Number.isSafeInteger(millis) ||
    millis < 0 ||
    !Number.isFinite(millis)
  ) {
    throw new CribraError("share_timestamp_invalid", { domain: "share" });
  }
  return { mode, generatedAtMillis: millis };
}

function manifestFrom(
  mode: number,
  generatedAtSeconds: bigint,
  generatedAtNanoseconds: number,
  summary: ShareManifest["summary"],
): ShareManifest {
  if (
    typeof generatedAtSeconds !== "bigint" ||
    generatedAtSeconds < 0n ||
    generatedAtSeconds > MAX_SAFE_SECONDS
  ) {
    throw new CribraError("share_timestamp_invalid", { domain: "share" });
  }
  if (
    !Number.isSafeInteger(generatedAtNanoseconds) ||
    generatedAtNanoseconds < 0 ||
    generatedAtNanoseconds >= NANOS_PER_SECOND
  ) {
    throw new CribraError("share_timestamp_invalid", { domain: "share" });
  }
  return Object.freeze({
    mode: shareModeFromNative(mode),
    generatedAtSeconds: Number(generatedAtSeconds),
    generatedAtNanoseconds,
    summary,
  });
}

/**
 * Scans and transforms an ordered source collection through Core's atomic
 * Share Bundle builder and returns detached DTOs.
 */
export function createShareBundle(
  scanner: Scanner,
  sources: readonly ScanSource[],
  options: ShareBundleOptions,
): ShareBundle {
  const internals = scannerInternals(scanner);
  if (internals === undefined) {
    throw invalidArgument();
  }
  if (!internals.isActive()) {
    throw new CribraError("disposed", { domain: "scanner" });
  }
  if (!internals.capabilities.shareBundle) {
    throw new CribraError("unsupported_capability", { domain: "capability" });
  }

  const normalized = normalizeOptions(options);
  const inputs = normalizeSources(sources);
  const wasm = internals.wasm as unknown as NativeShareModule;

  let builder: NativeShareBundleBuilder | undefined;
  try {
    builder = new wasm.ShareBundleBuilder();
    try {
      switch (normalized.mode.kind) {
        case "redact":
          builder.modeRedact();
          break;
        case "template":
          builder.modeTemplate();
          break;
        case "pseudonymize":
          builder.modePseudonymize(
            normalized.mode.key,
            normalized.mode.prefix,
            normalized.mode.digestBytes,
          );
          break;
        case "synthesize":
          builder.modeSynthesize(
            normalized.mode.key,
            normalized.mode.marker,
          );
          break;
      }
    } catch (error) {
      throw normalizeTransformError(error);
    }

    let bundle: NativeShareBundle;
    try {
      bundle =
        normalized.generatedAtMillis === undefined
          ? builder.scanAndBuild(internals.engine, inputs)
          : builder.scanAndBuildWithGeneratedAt(
              internals.engine,
              inputs,
              normalized.generatedAtMillis,
            );
    } catch (error) {
      throw normalizeShareBundleError(error);
    }

    try {
      const length = bundle.length;
      if (!Number.isSafeInteger(length) || length < 0) {
        throw new CribraError("share_unknown", { domain: "share" });
      }
      const entries: ShareBundleEntry[] = [];
      for (let index = 0; index < length; index += 1) {
        const entry = bundle.entryAt(index);
        try {
          entries.push(
            Object.freeze({ id: entry.id, content: entry.content }),
          );
        } finally {
          safeFree(entry);
        }
      }

      const manifestView = bundle.manifest();
      let manifest: ShareManifest;
      try {
        manifest = manifestFrom(
          manifestView.mode,
          manifestView.generatedAtSeconds,
          manifestView.generatedAtNanoseconds,
          projectSummary(manifestView.summary()),
        );
      } finally {
        safeFree(manifestView);
      }

      const summary = projectSummary(bundle.summary());
      return Object.freeze({
        entries: Object.freeze(entries),
        manifest,
        summary,
      });
    } finally {
      safeFree(bundle);
    }
  } finally {
    safeFree(builder);
  }
}
