/**
 * Pure request validation for the native Flow transformation API.
 *
 * This module never imports the TurboModule. It validates and normalizes the
 * caller request, resolves the documented Core defaults, and rejects malformed
 * UTF-16 before any native work is dispatched. Core remains the sole authority
 * for redaction, templating, pseudonymization and synthesis.
 */

import { CribraError } from './errors';
import { assertWellFormedUtf16 } from './offset';

/** Exact key size required by the C ABI and Core. */
export const TRANSFORM_KEY_BYTES = 32;

/**
 * Largest digest length representable on every supported native target and
 * accepted by the WebAssembly adapter. Core clamps the effective value to
 * 8..=32.
 */
const MAX_DIGEST_BYTES = 0xffff_ffff;

const DEFAULT_NAMESPACE = 'CRIBRA';
const DEFAULT_PREFIX = 'cribra_pseudo_';
const DEFAULT_DIGEST_BYTES = 16;
const DEFAULT_MARKER = 'cribra_synthetic';

export interface RedactRequest {
  readonly kind: 'redact';
  /** Omitted uses Core's "[REDACTED]". An empty string is a valid replacement. */
  readonly replacement?: string;
}

export interface TemplateRequest {
  readonly kind: 'template';
  /** Omitted uses Core's "CRIBRA". */
  readonly namespace?: string;
  readonly numbered?: boolean;
}

export interface PseudonymizeRequest {
  readonly kind: 'pseudonymize';
  /** Exactly 32 bytes. Copied for the call and never retained. */
  readonly key: Uint8Array;
  /** Omitted uses "cribra_pseudo_". */
  readonly prefix?: string;
  /** Nonnegative safe integer up to 0xFFFFFFFF; Core clamps to 8..=32. */
  readonly digestBytes?: number;
}

export interface SynthesizeRequest {
  readonly kind: 'synthesize';
  /** Exactly 32 bytes. Copied for the call and never retained. */
  readonly key: Uint8Array;
  /** Omitted uses "cribra_synthetic". */
  readonly marker?: string;
}

export type TransformRequest =
  | RedactRequest
  | TemplateRequest
  | PseudonymizeRequest
  | SynthesizeRequest;

/** Request with Core defaults resolved and option strings preflighted. */
export type NormalizedTransform =
  | { readonly kind: 'redact'; readonly replacement?: string }
  | {
      readonly kind: 'template';
      readonly namespace: string;
      readonly numbered: boolean;
    }
  | {
      readonly kind: 'pseudonymize';
      readonly key: Uint8Array;
      readonly prefix: string;
      readonly digestBytes: number;
    }
  | {
      readonly kind: 'synthesize';
      readonly key: Uint8Array;
      readonly marker: string;
    };

function invalidArgument(): CribraError {
  return new CribraError('invalid_argument');
}

function invalidKeyLength(): CribraError {
  return new CribraError('invalid_key_length');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function optionalString(
  source: Record<string, unknown>,
  field: string,
): string | undefined {
  const value = source[field];
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== 'string') {
    throw invalidArgument();
  }
  // Hermes and JSC do not agree on unpaired surrogates. Reject before native
  // work so the replacement/namespace/prefix/marker seen by Core is exactly the
  // string the caller supplied.
  assertWellFormedUtf16(value);
  return value;
}

function requireKey(value: unknown): Uint8Array {
  if (!(value instanceof Uint8Array) || value.byteLength !== TRANSFORM_KEY_BYTES) {
    throw invalidKeyLength();
  }
  return value;
}

function optionalDigestBytes(
  source: Record<string, unknown>,
): number | undefined {
  const value = source.digestBytes;
  if (value === undefined) {
    return undefined;
  }
  if (
    typeof value !== 'number' ||
    !Number.isSafeInteger(value) ||
    value < 0 ||
    value > MAX_DIGEST_BYTES
  ) {
    throw invalidArgument();
  }
  return value;
}

/**
 * Validates the complete request before any native call, resolves Core's
 * documented defaults, and returns an immutable normalized request.
 *
 * The key is validated but not copied here. The caller copies it immediately
 * before dispatch so the sensitive copy exists for the shortest practical time.
 */
export function normalizeTransformRequest(
  request: unknown,
): NormalizedTransform {
  if (!isRecord(request)) {
    throw invalidArgument();
  }

  switch (request.kind) {
    case 'redact': {
      const replacement = optionalString(request, 'replacement');
      return Object.freeze({
        kind: 'redact' as const,
        ...(replacement === undefined ? {} : { replacement }),
      });
    }
    case 'template': {
      const namespace = optionalString(request, 'namespace');
      const numbered = request.numbered;
      if (numbered !== undefined && typeof numbered !== 'boolean') {
        throw invalidArgument();
      }
      return Object.freeze({
        kind: 'template' as const,
        namespace: namespace ?? DEFAULT_NAMESPACE,
        numbered: numbered ?? false,
      });
    }
    case 'pseudonymize': {
      const key = requireKey(request.key);
      const prefix = optionalString(request, 'prefix');
      const digestBytes = optionalDigestBytes(request);
      return Object.freeze({
        kind: 'pseudonymize' as const,
        key,
        prefix: prefix ?? DEFAULT_PREFIX,
        digestBytes: digestBytes ?? DEFAULT_DIGEST_BYTES,
      });
    }
    case 'synthesize': {
      const key = requireKey(request.key);
      const marker = optionalString(request, 'marker');
      return Object.freeze({
        kind: 'synthesize' as const,
        key,
        marker: marker ?? DEFAULT_MARKER,
      });
    }
    default:
      throw invalidArgument();
  }
}

/**
 * Copies a validated key into a fresh, exactly sized array. The returned view
 * always owns a plain 32-byte ArrayBuffer, even when the caller supplied a
 * subarray of a larger or shared buffer.
 */
export function copyTransformKey(key: Uint8Array): Uint8Array {
  const copy = new Uint8Array(TRANSFORM_KEY_BYTES);
  try {
    copy.set(key);
  } catch {
    copy.fill(0);
    throw invalidKeyLength();
  }
  if (copy.byteLength !== TRANSFORM_KEY_BYTES) {
    copy.fill(0);
    throw invalidKeyLength();
  }
  return copy;
}

/** Best-effort wipe of a JavaScript-owned temporary. */
export function zeroTransformKey(key: Uint8Array): void {
  key.fill(0);
}
