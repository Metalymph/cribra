import assert from "node:assert/strict";
import test from "node:test";

import { loadPackage } from "../helpers.mjs";

test("byteSpanToTextRange handles ASCII", async () => {
  const api = await loadPackage();
  assert.deepEqual(api.byteSpanToTextRange("hello", { start: 0, end: 5 }), {
    start: 0,
    end: 5,
  });
  assert.deepEqual(api.byteSpanToTextRange("hello", { start: 1, end: 4 }), {
    start: 1,
    end: 4,
  });
});

test("byteSpanToTextRange handles accented scalars", async () => {
  const api = await loadPackage();
  const source = "héllo";
  assert.deepEqual(api.byteSpanToTextRange(source, { start: 0, end: 3 }), {
    start: 0,
    end: 2,
  });
  assert.deepEqual(api.byteSpanToTextRange(source, { start: 1, end: 6 }), {
    start: 1,
    end: 5,
  });
});

test("byteSpanToTextRange handles supplementary scalars", async () => {
  const api = await loadPackage();
  const source = "a🦀b";
  assert.deepEqual(api.byteSpanToTextRange(source, { start: 0, end: 5 }), {
    start: 0,
    end: 3,
  });
  assert.deepEqual(api.byteSpanToTextRange(source, { start: 1, end: 5 }), {
    start: 1,
    end: 3,
  });
  assert.deepEqual(api.byteSpanToTextRange(source, { start: 5, end: 6 }), {
    start: 3,
    end: 4,
  });
});

test("byteSpanToTextRange handles mixed scripts", async () => {
  const api = await loadPackage();
  const source = "é🦀a";
  assert.deepEqual(api.byteSpanToTextRange(source, { start: 0, end: 7 }), {
    start: 0,
    end: 4,
  });
  assert.deepEqual(api.byteSpanToTextRange(source, { start: 2, end: 6 }), {
    start: 1,
    end: 3,
  });
});

test("byteSpanToTextRange handles empty sources and spans", async () => {
  const api = await loadPackage();
  assert.deepEqual(api.byteSpanToTextRange("", { start: 0, end: 0 }), {
    start: 0,
    end: 0,
  });
  assert.deepEqual(api.byteSpanToTextRange("abc", { start: 0, end: 0 }), {
    start: 0,
    end: 0,
  });
  assert.deepEqual(api.byteSpanToTextRange("abc", { start: 3, end: 3 }), {
    start: 3,
    end: 3,
  });
});

test("byteSpanToTextRange rejects invalid values", async () => {
  const api = await loadPackage();
  const cases = [
    { start: -1, end: 0 },
    { start: 0, end: -1 },
    { start: 0.5, end: 1 },
    { start: 0, end: Number.NaN },
    { start: 0, end: Number.POSITIVE_INFINITY },
    { start: "0", end: 1 },
    { start: 0, end: undefined },
    {},
  ];
  for (const span of cases) {
    assert.throws(
      () => api.byteSpanToTextRange("abc", span),
      (error) => {
        assert.equal(error.name, "CribraError");
        assert.equal(error.code, "invalid_span_value");
        assert.equal(error.domain, "offset");
        return true;
      },
    );
  }
  assert.throws(
    () => api.byteSpanToTextRange("abc", null),
    (error) => error.code === "invalid_span_value",
  );
});

test("byteSpanToTextRange rejects reversed spans", async () => {
  const api = await loadPackage();
  assert.throws(
    () => api.byteSpanToTextRange("abc", { start: 2, end: 1 }),
    (error) => error.code === "invalid_span_order",
  );
});

test("byteSpanToTextRange rejects out-of-bounds spans", async () => {
  const api = await loadPackage();
  assert.throws(
    () => api.byteSpanToTextRange("abc", { start: 0, end: 4 }),
    (error) => error.code === "span_out_of_bounds",
  );
  assert.throws(
    () => api.byteSpanToTextRange("é", { start: 0, end: 3 }),
    (error) => error.code === "span_out_of_bounds",
  );
});

test("byteSpanToTextRange rejects non-scalar boundaries", async () => {
  const api = await loadPackage();
  for (const span of [
    { start: 1, end: 2 },
    { start: 0, end: 1 },
    { start: 1, end: 1 },
  ]) {
    assert.throws(
      () => api.byteSpanToTextRange("é", span),
      (error) => error.code === "span_not_utf8_boundary",
    );
  }
  assert.throws(
    () => api.byteSpanToTextRange("🦀", { start: 2, end: 2 }),
    (error) => error.code === "span_not_utf8_boundary",
  );
});

test("byteSpanToTextRange rejects unpaired surrogates before span checks", async () => {
  const api = await loadPackage();
  for (const source of ["\ud800", "\udc00", "a\ud800b", "a\udc00b"]) {
    assert.throws(
      () => api.byteSpanToTextRange(source, { start: -1, end: -2 }),
      (error) => error.code === "invalid_unicode_string",
    );
  }
});
