/** UTF-8 byte span -> UTF-16 code-unit range. Pure and native-independent. */

import { CribraError } from "./errors.js";
import type { ByteSpan, TextRange } from "./types.js";

function isSurrogate(codePoint: number): boolean {
  return codePoint >= 0xd800 && codePoint <= 0xdfff;
}

function utf8ScalarWidth(codePoint: number): number {
  if (codePoint <= 0x7f) return 1;
  if (codePoint <= 0x7ff) return 2;
  if (codePoint <= 0xffff) return 3;
  return 4;
}

export function byteSpanToTextRange(source: string, span: ByteSpan): TextRange {
  if (typeof source !== "string") {
    throw new CribraError("invalid_argument", {
      domain: "offset",
      message: "The offset source must be a string.",
    });
  }
  if (span === null || typeof span !== "object") {
    throw new CribraError("invalid_span_value");
  }

  const start: unknown = span.start;
  const end: unknown = span.end;

  // Walk every code point once. This both rejects malformed strings and
  // records exactly which UTF-8 byte offsets are scalar boundaries.
  const boundaries = new Map<number, number>();
  boundaries.set(0, 0);
  let bytes = 0;
  let index = 0;
  while (index < source.length) {
    const codePoint = source.codePointAt(index);
    if (codePoint === undefined) {
      throw new CribraError("invalid_unicode_string");
    }
    if (isSurrogate(codePoint)) {
      throw new CribraError("invalid_unicode_string");
    }
    bytes += utf8ScalarWidth(codePoint);
    index += codePoint > 0xffff ? 2 : 1;
    boundaries.set(bytes, index);
  }

  if (
    typeof start !== "number" ||
    typeof end !== "number" ||
    !Number.isSafeInteger(start) ||
    !Number.isSafeInteger(end) ||
    start < 0 ||
    end < 0
  ) {
    throw new CribraError("invalid_span_value");
  }
  if (start > end) {
    throw new CribraError("invalid_span_order");
  }
  if (end > bytes) {
    throw new CribraError("span_out_of_bounds");
  }

  const startIndex = boundaries.get(start);
  const endIndex = boundaries.get(end);
  if (startIndex === undefined || endIndex === undefined) {
    throw new CribraError("span_not_utf8_boundary");
  }
  return { start: startIndex, end: endIndex };
}
