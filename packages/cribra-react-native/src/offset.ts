/**
 * Exact UTF-8 byte span <-> UTF-16 code-unit range conversion.
 *
 * Pure and native-independent: no TurboModule, no `TextEncoder`, no native
 * call. Core returns zero-based half-open UTF-8 byte spans; JavaScript strings
 * are indexed in UTF-16 code units. This module is the only place where the
 * two coordinate systems are reconciled.
 *
 * Deterministic validation order, for every entry point:
 *   1. invalid source type        -> invalid_argument
 *   2. malformed UTF-16 source    -> invalid_unicode_string
 *   3. invalid span/range value   -> invalid_span_value
 *   4. reversed endpoints         -> invalid_span_order
 *   5. out-of-bounds endpoints    -> span_out_of_bounds
 *   6. non-scalar boundary        -> span_not_utf8_boundary
 *                                    span_not_utf16_boundary (reverse only)
 *
 * Conversions never clamp, round, substitute U+FFFD, or select half of a
 * surrogate pair. They either return an exact range or throw.
 */

import { CribraError } from './errors';
import type { CribraErrorCode } from './errors';

/** Half-open UTF-16 code-unit range, suitable for `String.prototype.slice`. */
export interface Utf16Range {
  readonly start: number;
  readonly end: number;
}

export interface ByteSpanLike {
  readonly start: number;
  readonly end: number;
}

function fail(code: CribraErrorCode): never {
  throw new CribraError(code);
}

/**
 * Decodes the scalar starting at `index` and returns `(units << 4) | width`,
 * where `width` is the exact UTF-8 byte length and `units` the UTF-16 code-unit
 * length. Malformed UTF-16 throws `invalid_unicode_string`.
 */
function scalarAt(source: string, index: number): number {
  const code = source.charCodeAt(index);
  if (code < 0x80) {
    return 0x11; // 1 unit, 1 byte
  }
  if (code < 0x800) {
    return 0x12; // 1 unit, 2 bytes
  }
  if (code < 0xd800) {
    return 0x13; // 1 unit, 3 bytes
  }
  if (code <= 0xdbff) {
    const next = index + 1 < source.length ? source.charCodeAt(index + 1) : 0;
    if (next < 0xdc00 || next > 0xdfff) {
      fail('invalid_unicode_string');
    }
    return 0x24; // 2 units, 4 bytes
  }
  if (code <= 0xdfff) {
    fail('invalid_unicode_string');
  }
  return 0x13; // 1 unit, 3 bytes
}

function isEndpoint(value: unknown): value is number {
  return (
    typeof value === 'number' &&
    Number.isSafeInteger(value) &&
    value >= 0
  );
}

/** Non-throwing endpoint read, so a malformed source always wins first. */
function peekEndpoint(container: unknown, key: 'start' | 'end'): number {
  if (container === null || typeof container !== 'object') {
    return Number.NaN;
  }
  const value = (container as Record<string, unknown>)[key];
  return typeof value === 'number' ? value : Number.NaN;
}

/** Structural and value validation, run after the source walk. */
function endpoint(container: unknown, key: 'start' | 'end'): number {
  if (container === null || typeof container !== 'object') {
    fail('invalid_span_value');
  }
  const value = (container as Record<string, unknown>)[key];
  if (!isEndpoint(value)) {
    fail('invalid_span_value');
  }
  return value;
}

/**
 * Rejects any unpaired high or low surrogate. This is the preflight used by the
 * public scan paths so that no native call ever sees a rewritten string.
 */
export function assertWellFormedUtf16(source: unknown): asserts source is string {
  if (typeof source !== 'string') {
    fail('invalid_argument');
  }
  for (let index = 0; index < source.length; index += 1) {
    const code = source.charCodeAt(index);
    if (code < 0xd800 || code > 0xdfff) {
      continue;
    }
    if (code <= 0xdbff && index + 1 < source.length) {
      const next = source.charCodeAt(index + 1);
      if (next >= 0xdc00 && next <= 0xdfff) {
        index += 1;
        continue;
      }
    }
    fail('invalid_unicode_string');
  }
}

/**
 * Exact UTF-8 byte length of `source`. Also rejects malformed UTF-16, so a
 * length comparison can never validate a string the native layer would rewrite.
 */
export function utf8ByteLength(source: unknown): number {
  if (typeof source !== 'string') {
    fail('invalid_argument');
  }
  let bytes = 0;
  let index = 0;
  while (index < source.length) {
    const packed = scalarAt(source, index);
    bytes += packed & 0x0f;
    index += packed >> 4;
  }
  return bytes;
}

/**
 * Converts a Core UTF-8 byte span into a UTF-16 code-unit range.
 *
 * One code-point walk: byte offsets increase strictly, so each requested
 * endpoint can be recorded at most once. Byte offset 0 and UTF-16 index 0 are
 * valid scalar boundaries for empty and non-empty sources.
 */
export function byteSpanToUtf16Range(
  source: unknown,
  span: ByteSpanLike,
): Utf16Range {
  if (typeof source !== 'string') {
    fail('invalid_argument');
  }

  // Read targets without throwing so malformed UTF-16 is reported first.
  const startTarget = peekEndpoint(span, 'start');
  const endTarget = peekEndpoint(span, 'end');

  let bytes = 0;
  let index = 0;
  let startIndex = startTarget === 0 ? 0 : -1;
  let endIndex = endTarget === 0 ? 0 : -1;

  while (index < source.length) {
    const packed = scalarAt(source, index);
    bytes += packed & 0x0f;
    index += packed >> 4;
    if (bytes === startTarget) {
      startIndex = index;
    }
    if (bytes === endTarget) {
      endIndex = index;
    }
  }

  const start = endpoint(span, 'start');
  const end = endpoint(span, 'end');
  if (start > end) {
    fail('invalid_span_order');
  }
  if (end > bytes) {
    fail('span_out_of_bounds');
  }
  if (startIndex < 0 || endIndex < 0) {
    fail('span_not_utf8_boundary');
  }
  return { start: startIndex, end: endIndex };
}

/**
 * Converts a UTF-16 code-unit range into a Core UTF-8 byte span.
 *
 * An endpoint that addresses the middle of a surrogate pair is rejected with
 * `span_not_utf16_boundary`; it is never rounded to a neighbouring scalar.
 */
export function utf16RangeToByteSpan(
  source: unknown,
  range: Utf16Range,
): ByteSpanLike {
  if (typeof source !== 'string') {
    fail('invalid_argument');
  }

  const startTarget = peekEndpoint(range, 'start');
  const endTarget = peekEndpoint(range, 'end');
  const length = source.length;

  let index = 0;
  let bytes = 0;
  let byteStart = startTarget === 0 ? 0 : -1;
  let byteEnd = endTarget === 0 ? 0 : -1;

  while (index < length) {
    const packed = scalarAt(source, index);
    if (index === startTarget) {
      byteStart = bytes;
    }
    if (index === endTarget) {
      byteEnd = bytes;
    }
    bytes += packed & 0x0f;
    index += packed >> 4;
  }
  // The exclusive end may be the end of the string, which the loop never visits
  // as a scalar start.
  if (startTarget === length) {
    byteStart = bytes;
  }
  if (endTarget === length) {
    byteEnd = bytes;
  }

  const start = endpoint(range, 'start');
  const end = endpoint(range, 'end');
  if (start > end) {
    fail('invalid_span_order');
  }
  if (end > length) {
    fail('span_out_of_bounds');
  }
  if (byteStart < 0 || byteEnd < 0) {
    fail('span_not_utf16_boundary');
  }
  return { start: byteStart, end: byteEnd };
}

/**
 * Converts many spans from one source in a single scalar walk.
 *
 * Time is O(n + k log k), auxiliary memory is O(k). Output order matches input
 * order, so duplicate, nested, and zero-length spans are supported. The whole
 * source is walked and validated before any span error can be reported.
 */
export function byteSpansToUtf16Ranges(
  source: unknown,
  spans: readonly ByteSpanLike[],
): readonly Utf16Range[] {
  if (typeof source !== 'string') {
    fail('invalid_argument');
  }

  // Collect readable numeric targets without throwing, so the whole source is
  // validated for malformed UTF-16 before any span error can surface.
  const isArray = Array.isArray(spans);
  const targets = new Set<number>();
  if (isArray) {
    for (const span of spans) {
      const start = peekEndpoint(span, 'start');
      const end = peekEndpoint(span, 'end');
      if (Number.isFinite(start)) {
        targets.add(start);
      }
      if (Number.isFinite(end)) {
        targets.add(end);
      }
    }
  }

  const ordered = Array.from(targets).sort((a, b) => a - b);
  const resolved = new Map<number, number>();
  let cursor = 0;
  if (ordered.length > 0 && ordered[0] === 0) {
    resolved.set(0, 0);
    cursor = 1;
  }

  // One walk over the whole source: validates UTF-16 and resolves every
  // distinct boundary. It runs to the end even when all targets are known.
  let bytes = 0;
  let index = 0;
  while (index < source.length) {
    const packed = scalarAt(source, index);
    bytes += packed & 0x0f;
    index += packed >> 4;
    while (cursor < ordered.length && ordered[cursor] === bytes) {
      resolved.set(ordered[cursor], index);
      cursor += 1;
    }
  }

  if (!isArray) {
    fail('invalid_span_value');
  }

  // Category-global validation order across the whole batch: every value first,
  // then every reversed pair, then every out-of-bounds endpoint.
  const bounds: { start: number; end: number }[] = [];
  for (const span of spans) {
    bounds.push({ start: endpoint(span, 'start'), end: endpoint(span, 'end') });
  }
  for (const bound of bounds) {
    if (bound.start > bound.end) {
      fail('invalid_span_order');
    }
  }
  for (const bound of bounds) {
    if (bound.end > bytes) {
      fail('span_out_of_bounds');
    }
  }

  const ranges: Utf16Range[] = [];
  for (const bound of bounds) {
    const start = resolved.get(bound.start);
    const end = resolved.get(bound.end);
    if (start === undefined || end === undefined) {
      fail('span_not_utf8_boundary');
    }
    ranges.push({ start, end });
  }
  return ranges;
}
