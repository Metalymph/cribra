/**
 * Deterministic RN.1.4 Unicode boundary tests.
 *
 * These are pure: they import `src/offset` directly, never the TurboModule, so
 * no native mock is required. Every expected range is a direct literal, not a
 * value produced by the implementation under test.
 */

jest.mock('../../src/NativeCribraReactNative', () => {
  throw new Error('offset tests must not load the native module');
});

import { CribraError } from '../../src/errors';
import {
  byteSpanToUtf16Range,
  byteSpansToUtf16Ranges,
  utf16RangeToByteSpan,
  utf8ByteLength,
} from '../../src/offset';

function expectCode(run: () => unknown, code: string): CribraError {
  try {
    run();
  } catch (error) {
    expect(error).toBeInstanceOf(CribraError);
    const cribra = error as CribraError;
    expect(cribra.code).toBe(code);
    return cribra;
  }
  throw new Error(`Expected CribraError(${code}) but nothing was thrown.`);
}

const FORWARD: readonly (readonly [
  string,
  [number, number],
  [number, number],
])[] = [
  ['hello', [0, 5], [0, 5]],
  ['hello', [1, 4], [1, 4]],
  ['hello', [0, 0], [0, 0]],
  ['héllo', [0, 3], [0, 2]],
  ['héllo', [1, 3], [1, 2]],
  ['héllo', [1, 6], [1, 5]],
  ['a🦀b', [0, 5], [0, 3]],
  ['a🦀b', [1, 5], [1, 3]],
  ['a🦀b', [5, 6], [3, 4]],
  ['a🦀b', [0, 6], [0, 4]],
  ['e\u0301', [0, 3], [0, 2]],
  ['e\u0301', [1, 3], [1, 2]],
  ['👨‍👩‍👧‍👦', [0, 25], [0, 11]],
  ['👨‍👩‍👧‍👦', [4, 7], [2, 3]],
  ['👨‍👩‍👧‍👦', [11, 18], [5, 8]],
  ['前\r\né', [0, 3], [0, 1]],
  ['前\r\né', [3, 5], [1, 3]],
  ['前\r\né', [3, 4], [1, 2]],
  ['前\r\né', [0, 7], [0, 4]],
  ['', [0, 0], [0, 0]],
  ['\r\n', [0, 2], [0, 2]],
  ['a\u0000b', [0, 3], [0, 3]],
];

describe('byteSpanToUtf16Range', () => {
  it.each(FORWARD)('%j %j -> %j', (source, span, expected) => {
    expect(
      byteSpanToUtf16Range(source, { start: span[0], end: span[1] }),
    ).toEqual({ start: expected[0], end: expected[1] });
  });

  it('never splits a four-byte scalar', () => {
    for (const span of [
      { start: 1, end: 2 },
      { start: 2, end: 5 },
      { start: 2, end: 2 },
    ]) {
      expectCode(() => byteSpanToUtf16Range('a🦀b', span), 'span_not_utf8_boundary');
    }
  });

  it('accepts scalar boundaries inside a combining grapheme', () => {
    expect(byteSpanToUtf16Range('e\u0301', { start: 1, end: 3 })).toEqual({
      start: 1,
      end: 2,
    });
    expect(byteSpanToUtf16Range('e\u0301', { start: 1, end: 1 })).toEqual({
      start: 1,
      end: 1,
    });
  });
});

const REVERSE: readonly (readonly [
  string,
  [number, number],
  [number, number],
])[] = [
  ['hello', [0, 5], [0, 5]],
  ['héllo', [0, 2], [0, 3]],
  ['héllo', [1, 5], [1, 6]],
  ['a🦀b', [0, 3], [0, 5]],
  ['a🦀b', [1, 3], [1, 5]],
  ['a🦀b', [3, 4], [5, 6]],
  ['e\u0301', [0, 2], [0, 3]],
  ['e\u0301', [1, 2], [1, 3]],
  ['👨‍👩‍👧‍👦', [0, 11], [0, 25]],
  ['👨‍👩‍👧‍👦', [2, 3], [4, 7]],
  ['前\r\né', [0, 4], [0, 7]],
  ['前\r\né', [1, 3], [3, 5]],
  ['', [0, 0], [0, 0]],
  ['\r\n', [0, 2], [0, 2]],
];

describe('utf16RangeToByteSpan', () => {
  it.each(REVERSE)('%j %j -> %j', (source, range, expected) => {
    expect(
      utf16RangeToByteSpan(source, { start: range[0], end: range[1] }),
    ).toEqual({ start: expected[0], end: expected[1] });
  });

  it('rejects an endpoint inside a surrogate pair', () => {
    expectCode(() => utf16RangeToByteSpan('🙂', { start: 0, end: 1 }), 'span_not_utf16_boundary');
    expectCode(() => utf16RangeToByteSpan('🙂', { start: 1, end: 1 }), 'span_not_utf16_boundary');
    expectCode(() => utf16RangeToByteSpan('🙂', { start: 1, end: 2 }), 'span_not_utf16_boundary');
  });

  it('accepts index zero and the exclusive end for non-empty sources', () => {
    expect(utf16RangeToByteSpan('a🙂b', { start: 0, end: 0 })).toEqual({
      start: 0,
      end: 0,
    });
    expect(utf16RangeToByteSpan('a🙂b', { start: 4, end: 4 })).toEqual({
      start: 6,
      end: 6,
    });
  });
});

/** Independent scalar boundary table, built without the module under test. */
function boundaries(value: string): { bytes: number; units: number }[] {
  const rows = [{ bytes: 0, units: 0 }];
  let bytes = 0;
  let units = 0;
  for (const character of value) {
    const codePoint = character.codePointAt(0) as number;
    bytes +=
      codePoint <= 0x7f
        ? 1
        : codePoint <= 0x7ff
          ? 2
          : codePoint <= 0xffff
            ? 3
            : 4;
    units += character.length;
    rows.push({ bytes, units });
  }
  return rows;
}

const ROUND_TRIP = [
  '',
  'hello',
  'héllo',
  '前\r\né',
  'a🦀b',
  'e\u0301',
  '👨‍👩‍👧‍👦',
  '🙂🙂🙂',
  'mixed 前 é 🙂 e\u0301\r\nend',
];

describe('round-trip invariants', () => {
  it.each(ROUND_TRIP)('%j covers every scalar boundary pair', (source) => {
    const table = boundaries(source);
    for (const from of table) {
      for (const to of table) {
        if (from.units > to.units) {
          continue;
        }
        const range = byteSpanToUtf16Range(source, {
          start: from.bytes,
          end: to.bytes,
        });
        expect(range).toEqual({ start: from.units, end: to.units });
        expect(utf16RangeToByteSpan(source, range)).toEqual({
          start: from.bytes,
          end: to.bytes,
        });
      }
    }
  });
});

describe('byteSpansToUtf16Ranges', () => {
  const source = '前\r\né🙂e\u0301 a🦀b';

  it('preserves input order for shuffled, duplicate, nested and empty spans', () => {
    const spans = [
      { start: 5, end: 11 },
      { start: 0, end: 0 },
      { start: 5, end: 11 },
      { start: 7, end: 7 },
      { start: 0, end: 3 },
      { start: 3, end: 5 },
    ];
    const ranges = byteSpansToUtf16Ranges(source, spans);
    expect(ranges).toEqual(
      spans.map((span) => byteSpanToUtf16Range(source, span)),
    );
    expect(ranges[0]).toEqual(ranges[2]);
  });

  it('handles an empty span list', () => {
    expect(byteSpansToUtf16Ranges('abc', [])).toEqual([]);
    expect(byteSpansToUtf16Ranges('', [])).toEqual([]);
  });

  it('converts the whole source', () => {
    const bytes = utf8ByteLength(source);
    expect(
      byteSpansToUtf16Ranges(source, [{ start: 0, end: bytes }]),
    ).toEqual([{ start: 0, end: source.length }]);
  });
});

describe('utf8ByteLength', () => {
  it.each([
    ['', 0],
    ['hello', 5],
    ['é', 2],
    ['Ā', 2],
    ['前', 3],
    ['🙂', 4],
    ['e\u0301', 3],
    ['前\r\né🙂e\u0301', 14],
  ])('%j -> %i bytes', (source, expected) => {
    expect(utf8ByteLength(source)).toBe(expected);
  });

  it('distinct strings can share a UTF-8 byte length', () => {
    expect(utf8ByteLength('é')).toBe(2);
    expect(utf8ByteLength('Ā')).toBe(2);
    expect('é').not.toBe('Ā');

    // A byte-length match is a consistency check, not an identity check: the
    // same span means different text in each string.
    expect(byteSpanToUtf16Range('é', { start: 0, end: 2 })).toEqual({
      start: 0,
      end: 1,
    });
    expect(byteSpanToUtf16Range('Ā', { start: 0, end: 2 })).toEqual({
      start: 0,
      end: 1,
    });
    expect('é'.slice(0, 1)).not.toBe('Ā'.slice(0, 1));
  });
});

describe('validation order', () => {
  const malformed = 'good\uD83Dbad';

  it('reports a malformed source before an invalid span', () => {
    expectCode(
      () => byteSpanToUtf16Range(malformed, null as never),
      'invalid_unicode_string',
    );
    expectCode(
      () => byteSpanToUtf16Range(malformed, { start: 9, end: 1 }),
      'invalid_unicode_string',
    );
    expectCode(
      () => byteSpanToUtf16Range(malformed, { start: 0, end: 999 }),
      'invalid_unicode_string',
    );
  });

  it('reports a malformed source before an invalid range', () => {
    expectCode(
      () => utf16RangeToByteSpan(malformed, null as never),
      'invalid_unicode_string',
    );
    expectCode(
      () => utf16RangeToByteSpan(malformed, { start: 4, end: 2 }),
      'invalid_unicode_string',
    );
  });

  it('reports a malformed source before any batch span error', () => {
    expectCode(
      () => byteSpansToUtf16Ranges(malformed, [null as never]),
      'invalid_unicode_string',
    );
    expectCode(
      () => byteSpansToUtf16Ranges(malformed, [
        { start: 0, end: 1 },
        { start: 9, end: 2 },
      ]),
      'invalid_unicode_string',
    );
  });

  it('rejects unpaired high and low surrogates in both directions', () => {
    for (const source of ['\uD83D', '\uDC00', 'a\uD83D', '\uDC00a']) {
      expectCode(
        () => byteSpanToUtf16Range(source, { start: 0, end: 0 }),
        'invalid_unicode_string',
      );
      expectCode(
        () => utf16RangeToByteSpan(source, { start: 0, end: 0 }),
        'invalid_unicode_string',
      );
      expectCode(
        () => byteSpansToUtf16Ranges(source, []),
        'invalid_unicode_string',
      );
    }
  });

  it('applies category precedence across the whole batch', () => {
    // A reversed first span must not mask an invalid value in a later span.
    expectCode(
      () =>
        byteSpansToUtf16Ranges('hello', [
          { start: 5, end: 4 },
          { start: -1, end: 2 },
        ]),
      'invalid_span_value',
    );
    // A later reversed span outranks an earlier out-of-bounds span.
    expectCode(
      () =>
        byteSpansToUtf16Ranges('hello', [
          { start: 0, end: 9 },
          { start: 4, end: 3 },
        ]),
      'invalid_span_order',
    );
    // Out-of-bounds outranks a non-scalar boundary in an earlier span.
    expectCode(
      () =>
        byteSpansToUtf16Ranges('a🙂b', [
          { start: 2, end: 2 },
          { start: 0, end: 9 },
        ]),
      'span_out_of_bounds',
    );
  });

  it('keeps the remaining precedence deterministic', () => {
    expectCode(
      () => byteSpanToUtf16Range(42 as never, { start: 0, end: 0 }),
      'invalid_argument',
    );
    expectCode(
      () => byteSpanToUtf16Range('hello', null as never),
      'invalid_span_value',
    );
    expectCode(
      () => byteSpanToUtf16Range('hello', { start: -1, end: 2 }),
      'invalid_span_value',
    );
    expectCode(
      () => byteSpanToUtf16Range('hello', { start: 0.5, end: 2 }),
      'invalid_span_value',
    );
    expectCode(
      () => byteSpanToUtf16Range('hello', { start: Number.NaN, end: 2 }),
      'invalid_span_value',
    );
    expectCode(
      () => byteSpanToUtf16Range('hello', { start: 2 ** 53, end: 2 }),
      'invalid_span_value',
    );
    expectCode(
      () => byteSpanToUtf16Range('hello', { start: 5, end: 4 }),
      'invalid_span_order',
    );
    expectCode(
      () => byteSpanToUtf16Range('hello', { start: 0, end: 6 }),
      'span_out_of_bounds',
    );
    expectCode(
      () => byteSpanToUtf16Range('hello', { start: 1, end: 6 }),
      'span_out_of_bounds',
    );
    expectCode(
      () => utf16RangeToByteSpan('ab', { start: 3, end: 3 }),
      'span_out_of_bounds',
    );
    expectCode(
      () => utf16RangeToByteSpan('ab', { start: 2, end: 1 }),
      'invalid_span_order',
    );
  });
});

describe('privacy', () => {
  it('keeps messages constant and source-free', () => {
    const secret = 'sk_live_SECRET_VALUE\uD83D';
    const error = expectCode(
      () => byteSpanToUtf16Range(secret, { start: 0, end: 0 }),
      'invalid_unicode_string',
    );
    expect(error.message).toBe(
      'A Cribra string contains an unpaired UTF-16 surrogate.',
    );
    const serialized = `${error.message}${error.detail ?? ''}${error.ruleId ?? ''}`;
    expect(serialized).not.toContain('sk_live');
    expect(serialized).not.toContain('SECRET');
    expect(serialized).not.toContain('surrogate at');
    expect(serialized).not.toContain('0');
  });
});
