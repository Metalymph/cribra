import assert from "node:assert/strict";
import { before, test } from "node:test";

import {
  customScannerOptions,
  loadPackage,
  privacySentinel,
} from "../helpers.mjs";

let api;

before(async () => {
  api = await loadPackage();
  await api.init();
});

function expectCribra(error, code, domain) {
  assert.equal(error.name, "CribraError");
  assert.equal(error.code, code);
  if (domain !== undefined) assert.equal(error.domain, domain);
  assert.equal("cause" in error, false, "cause is omitted");
  assert.equal("free" in error, false, "no native handle on errors");
  return true;
}

function assertScannerStillWorks() {
  const scanner = api.createScanner(customScannerOptions());
  try {
    assert.equal(scanner.scan("token-1234").findingCount, 1);
  } finally {
    scanner.dispose();
  }
}

test("invalid scanner options are rejected without corrupting later scanners", () => {
  const cases = [
    { builtins: "bogus" },
    { financialBuiltins: "yes" },
    { personalBuiltins: 1 },
    { rules: "nope" },
    { rules: [null] },
    { rules: [{ kind: "bogus", id: "x", severity: "high", value: "x" }] },
    { rules: [{ kind: "literal", id: "x", severity: "bogus", value: "x" }] },
    {
      rules: [
        {
          kind: "literal",
          id: "x",
          severity: "high",
          value: "x",
          remediation: "bogus",
        },
      ],
    },
    { rules: [{ kind: "literal", id: "x", severity: "high" }] },
    { rules: [{ kind: "pattern", id: "x", severity: "high" }] },
    { rules: [{ kind: "pattern", id: "x", pattern: "x", severity: "high", captureGroup: 0 }] },
  ];
  for (const options of cases) {
    assert.throws(
      () => api.createScanner(options),
      (error) =>
        options.rules?.[0]?.captureGroup !== undefined
          ? expectCribra(error, "unsupported_capability", "capability")
          : expectCribra(error, "invalid_argument", "scanner"),
    );
    assertScannerStillWorks();
  }
});

test("adapter build errors map to stable codes and preserve ruleId", () => {
  const cases = [
    {
      rules: [
        { id: "bad.pattern", kind: "pattern", pattern: "(", severity: "high" },
      ],
      code: "invalid_pattern",
      ruleId: "bad.pattern",
    },
    {
      rules: [
        {
          id: "empty.pattern",
          kind: "pattern",
          pattern: "a*",
          severity: "high",
        },
      ],
      code: "pattern_matches_empty",
      ruleId: "empty.pattern",
    },
    {
      rules: [{ id: "", kind: "literal", value: "x", severity: "high" }],
      code: "empty_rule_id",
    },
    {
      rules: [
        { id: "dup", kind: "literal", value: "x", severity: "high" },
        { id: "dup", kind: "literal", value: "y", severity: "low" },
      ],
      code: "duplicate_rule_id",
      ruleId: "dup",
    },
    {
      rules: [{ id: "empty.matcher", kind: "literal", value: "", severity: "high" }],
      code: "empty_matcher",
      ruleId: "empty.matcher",
    },
    {
      rules: [
        {
          id: "bad.remediation",
          kind: "literal",
          value: "x",
          severity: "high",
          remediation: "unknown",
        },
      ],
      code: "invalid_remediation",
      ruleId: "bad.remediation",
    },
  ];

  for (const entry of cases) {
    assert.throws(
      () => api.createScanner({ builtins: "none", rules: entry.rules }),
      (error) => {
        expectCribra(error, entry.code, "scanner");
        if (entry.ruleId !== undefined) {
          assert.equal(error.ruleId, entry.ruleId);
        }
        return true;
      },
    );
    assertScannerStillWorks();
  }
});

test("scan and batch argument errors are structured and recoverable", () => {
  const scanner = api.createScanner(customScannerOptions());
  try {
    assert.throws(
      () => scanner.scan(123),
      (error) => expectCribra(error, "invalid_argument", "scanner"),
    );
    assert.throws(
      () => scanner.scan("token-1234", { includeExplanations: "yes" }),
      (error) => expectCribra(error, "invalid_argument", "scanner"),
    );
    assert.throws(
      () => scanner.scanBatch("nope"),
      (error) => expectCribra(error, "invalid_argument", "scanner"),
    );
    assert.throws(
      () => scanner.scanBatch([{ id: "x" }]),
      (error) => expectCribra(error, "invalid_argument", "scanner"),
    );
    assert.throws(
      () => scanner.scanBatch([{ id: "x", source: "token-1234" }], { includeExplanations: 1 }),
      (error) => expectCribra(error, "invalid_argument", "scanner"),
    );

    assert.equal(scanner.scan("token-1234").findingCount, 1);
    assert.equal(
      scanner.scanBatch([{ id: "x", source: "token-1234" }]).summary.totalFindings,
      1,
    );
  } finally {
    scanner.dispose();
  }
});

test("disposed scanners fail with disposed and disposal is idempotent", () => {
  const scanner = api.createScanner(customScannerOptions());
  scanner.dispose();
  scanner.dispose();
  assert.equal(scanner.disposed, true);
  assert.throws(
    () => scanner.scan("token-1234"),
    (error) => expectCribra(error, "disposed", "scanner"),
  );
  assert.throws(
    () => scanner.scanBatch([]),
    (error) => expectCribra(error, "disposed", "scanner"),
  );
  assert.throws(
    () => scanner.transform("token-1234", { kind: "redact" }),
    (error) => expectCribra(error, "disposed", "scanner"),
  );
  assert.throws(
    () => api.createShareBundle(scanner, [], { mode: { kind: "redact" } }),
    (error) => expectCribra(error, "disposed", "scanner"),
  );
  assert.equal(scanner.rulesCount, 4);
});

test("errors and results do not leak scanned source or matcher payloads", () => {
  const scanner = api.createScanner({
    builtins: "none",
    rules: [
      {
        id: "privacy.literal",
        kind: "literal",
        value: privacySentinel,
        severity: "high",
      },
    ],
  });
  try {
    const source = `before ${privacySentinel} after`;
    const result = scanner.scan(source);
    assert.equal(result.findingCount, 1);
    assert.equal(JSON.stringify(result).includes(privacySentinel), false);
    assert.equal(JSON.stringify(result).includes(source), false);

    assert.throws(
      () =>
        api.createScanner({
          builtins: "none",
          rules: [
            {
              id: "privacy.pattern",
              kind: "pattern",
              pattern: `(${privacySentinel}`,
              severity: "high",
            },
          ],
        }),
      (error) => {
        expectCribra(error, "invalid_pattern", "scanner");
        assert.equal(error.message.includes(privacySentinel), false);
        assert.equal(JSON.stringify(error).includes(privacySentinel), false);
        assert.equal(String(error).includes(privacySentinel), false);
        return true;
      },
    );
  } finally {
    scanner.dispose();
  }
});
