/** Scan-coupled transformations. Exactly one native scan per call. */

import { CribraError } from "./errors.js";
import { normalizeTransformError } from "./native-errors.js";
import { projectScanResult } from "./result.js";
import { isRecord, MAX_WASM_USIZE, safeFree } from "./session.js";
import type {
  Capabilities,
  PseudonymizeRequest,
  RedactRequest,
  SynthesizeRequest,
  TemplateRequest,
  TransformRequest,
  TransformResult,
} from "./types.js";
import type { ScanEngine, ScanResult as NativeScanResult } from "./wasm/cribra.js";

/** Flow-only methods are absent from Core declarations, so keep them structural. */
interface NativeTransformResult {
  redact(source: string): string;
  redactWith(source: string, replacement: string): string;
  template(source: string): string;
  templateWith(source: string, namespace: string, numbered: boolean): string;
  pseudonymize(
    source: string,
    key: Uint8Array,
    prefix: string,
    digestBytes: number,
  ): string;
  synthesize(source: string, key: Uint8Array, marker: string): string;
}

const DEFAULT_REPLACEMENT = "[REDACTED]";
const DEFAULT_NAMESPACE = "CRIBRA";
const DEFAULT_PREFIX = "cribra_pseudo_";
const DEFAULT_DIGEST_BYTES = 16;
const DEFAULT_MARKER = "cribra_synthetic";
const KEY_BYTES = 32;

function invalidArgument(): CribraError {
  return new CribraError("invalid_argument", { domain: "transform" });
}

function invalidKeyLength(): CribraError {
  return new CribraError("invalid_key_length", { domain: "transform" });
}

function capabilityFor(
  capabilities: Capabilities,
  kind: TransformRequest["kind"],
): boolean {
  switch (kind) {
    case "redact":
      return capabilities.redaction;
    case "template":
      return capabilities.templating;
    case "pseudonymize":
      return capabilities.pseudonymization;
    case "synthesize":
      return capabilities.synthesis;
  }
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

function normalizeRedact(request: Record<string, unknown>): RedactRequest {
  const replacement = optionalString(request, "replacement");
  return {
    kind: "redact",
    ...(replacement === undefined ? {} : { replacement }),
  };
}

function normalizeTemplate(request: Record<string, unknown>): TemplateRequest {
  const numbered = request.numbered;
  if (numbered !== undefined && typeof numbered !== "boolean") {
    throw invalidArgument();
  }
  const namespace = optionalString(request, "namespace");
  return {
    kind: "template",
    ...(namespace === undefined ? {} : { namespace }),
    ...(numbered === undefined ? {} : { numbered }),
  };
}

function normalizePseudonymize(
  request: Record<string, unknown>,
): PseudonymizeRequest {
  const key = requireKey(request.key);
  const prefix = optionalString(request, "prefix");
  const digestBytes = optionalDigestBytes(request);
  return {
    kind: "pseudonymize",
    key,
    ...(prefix === undefined ? {} : { prefix }),
    ...(digestBytes === undefined ? {} : { digestBytes }),
  };
}

function normalizeSynthesize(
  request: Record<string, unknown>,
): SynthesizeRequest {
  const key = requireKey(request.key);
  const marker = optionalString(request, "marker");
  return {
    kind: "synthesize",
    key,
    ...(marker === undefined ? {} : { marker }),
  };
}

/** Validates the complete request before any native scan occurs. */
export function normalizeTransformRequest(request: unknown): TransformRequest {
  if (!isRecord(request)) throw invalidArgument();
  switch (request.kind) {
    case "redact":
      return normalizeRedact(request);
    case "template":
      return normalizeTemplate(request);
    case "pseudonymize":
      return normalizePseudonymize(request);
    case "synthesize":
      return normalizeSynthesize(request);
    default:
      throw invalidArgument();
  }
}

export function performTransform(
  engine: ScanEngine,
  capabilities: Capabilities,
  source: string,
  request: TransformRequest,
): TransformResult {
  if (!capabilityFor(capabilities, request.kind)) {
    throw new CribraError("unsupported_capability", { domain: "capability" });
  }

  let native: NativeScanResult;
  try {
    native = engine.scan(source);
  } catch {
    throw new CribraError("transform_unknown", { domain: "transform" });
  }

  try {
    const scan = projectScanResult(engine, native, false);
    const transform = native as unknown as NativeTransformResult;
    let output: string;
    switch (request.kind) {
      case "redact":
        output =
          request.replacement === undefined
            ? transform.redact(source)
            : transform.redactWith(source, request.replacement);
        break;
      case "template":
        output = transform.templateWith(
          source,
          request.namespace ?? DEFAULT_NAMESPACE,
          request.numbered ?? false,
        );
        break;
      case "pseudonymize":
        output = transform.pseudonymize(
          source,
          request.key,
          request.prefix ?? DEFAULT_PREFIX,
          request.digestBytes ?? DEFAULT_DIGEST_BYTES,
        );
        break;
      case "synthesize":
        output = transform.synthesize(
          source,
          request.key,
          request.marker ?? DEFAULT_MARKER,
        );
        break;
      default:
        throw invalidArgument();
    }
    return Object.freeze({ scan, output });
  } catch (error) {
    throw normalizeTransformError(error);
  } finally {
    safeFree(native);
  }
}
