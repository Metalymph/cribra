/**
 * RN.1.4 direct-Core span correspondence.
 *
 * Uses the committed direct-Core oracle fixture: every finding and candidate
 * byte span is converted with the pure TypeScript layer, then checked against
 * the exact source string. This proves UTF-8 byte <-> UTF-16 code-unit
 * correspondence without a native runtime and without trusting either the
 * native DTO or the oracle for string lengths.
 */

jest.mock('../../src/NativeCribraReactNative', () => {
  throw new Error('oracle tests must not load the native module');
});

import {
  byteSpanToUtf16Range,
  byteSpansToUtf16Ranges,
  utf16RangeToByteSpan,
  utf8ByteLength,
} from '../../src/offset';

interface ByteSpan {
  readonly start: number;
  readonly end: number;
}

interface OracleEntry {
  readonly id: string;
  readonly sourceBytes: number;
  readonly result: {
    readonly sourceBytes: number;
    readonly findings: readonly { readonly span: ByteSpan }[];
    readonly candidates: readonly { readonly span: ByteSpan }[];
  };
}

const sources = require('../src/fixtures/synthetic-source.json') as {
  id: string;
  text: string;
}[];
const oracle = require('../src/fixtures/synthetic-oracle.json') as {
  entries: readonly OracleEntry[];
};

function assertWholeScalarEdges(
  text: string,
  range: { start: number; end: number },
): void {
  if (range.start > 0) {
    const previous = text.charCodeAt(range.start - 1);
    const current = text.charCodeAt(range.start);
    const splitsPair =
      previous >= 0xd800 &&
      previous <= 0xdbff &&
      current >= 0xdc00 &&
      current <= 0xdfff;
    expect(splitsPair).toBe(false);
  }
  if (range.end > 0 && range.end < text.length) {
    const current = text.charCodeAt(range.end - 1);
    const next = text.charCodeAt(range.end);
    const splitsPair =
      current >= 0xd800 &&
      current <= 0xdbff &&
      next >= 0xdc00 &&
      next <= 0xdfff;
    expect(splitsPair).toBe(false);
  }
}

describe('direct-Core oracle correspondence', () => {
  it('has one source per oracle entry', () => {
    expect(oracle.entries).toHaveLength(sources.length);
  });

  it('pins the known fixture span exactly', () => {
    // "前\r\né🙂e\u0301 GITHUB_TOKEN=..." — the first finding starts after
    // 22 UTF-16 code units, not 28.
    expect(oracle.entries[0].result.findings[0].span).toEqual({
      start: 28,
      end: 65,
    });
    expect(
      byteSpanToUtf16Range(sources[0].text, { start: 28, end: 65 }),
    ).toEqual({ start: 22, end: 59 });
  });

  for (const [index, entry] of oracle.entries.entries()) {
    it(`converts entry ${index} (${entry.id.length} code units in id)`, () => {
      const text = sources[index].text;
      expect(sources[index].id).toBe(entry.id);
      expect(utf8ByteLength(text)).toBe(entry.sourceBytes);
      expect(entry.result.sourceBytes).toBe(entry.sourceBytes);

      const spans = [
        ...entry.result.findings.map((finding) => finding.span),
        ...entry.result.candidates.map((candidate) => candidate.span),
      ];

      for (const span of spans) {
        const range = byteSpanToUtf16Range(text, span);
        expect(utf8ByteLength(text.slice(range.start, range.end))).toBe(
          span.end - span.start,
        );
        expect(utf16RangeToByteSpan(text, range)).toEqual({
          start: span.start,
          end: span.end,
        });
        expect(text.slice(range.start, range.end)).toBe(
          text.substring(range.start, range.end),
        );
        assertWholeScalarEdges(text, range);
      }

      // Batch conversion must agree with single-span conversion in order.
      expect(byteSpansToUtf16Ranges(text, spans)).toEqual(
        spans.map((span) => byteSpanToUtf16Range(text, span)),
      );
    });
  }
});
