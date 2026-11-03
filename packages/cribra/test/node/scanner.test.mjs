import assert from "node:assert/strict";
import { before, test } from "node:test";

import {
  candidateSource,
  customRules,
  customScannerOptions,
  customSource,
  githubToken,
  loadPackage,
  occurrenceSpan,
  plainData,
} from "../helpers.mjs";

let api;

before(async () => {
  api = await loadPackage();
  await api.init();
});

test("default scanner uses the built-in catalog", () => {
  const scanner = api.createScanner();
  try {
    assert.equal(scanner.rules.length, scanner.rulesCount);
    assert.ok(scanner.rulesCount > 0);
    const second = api.createScanner();
    try {
      assert.deepEqual(
        scanner.rules,
        second.rules,
        "built-in catalog order is deterministic",
      );
    } finally {
      second.dispose();
    }

    const github = scanner.rules.find(
      (rule) => rule.ruleId === "github.classic-pat",
    );
    assert.deepEqual(github, {
      ruleId: "github.classic-pat",
      kind: "prefix",
      detectionMode: "deterministic",
      severity: "critical",
      remediation: "revoke_and_rotate_credential",
    });

    const result = scanner.scan(`GITHUB_TOKEN=${githubToken}`);
    const finding = result.findings.find(
      (entry) => entry.ruleId === "github.classic-pat",
    );
    assert.ok(finding, "built-in rule produces a finding");
    assert.deepEqual(finding.span, occurrenceSpan(`GITHUB_TOKEN=${githubToken}`, githubToken));
    assert.equal(finding.severity, "critical");
    assert.equal(finding.remediation, "revoke_and_rotate_credential");
    assert.equal(finding.confidence, "high");
    assert.equal("explanation" in finding, false);
  } finally {
    scanner.dispose();
  }
});

test("custom-only scanner does not include built-ins", () => {
  const scanner = api.createScanner(customScannerOptions());
  try {
    assert.equal(scanner.rulesCount, customRules.length);
    assert.deepEqual(scanner.rules, [
      {
        ruleId: "test.literal",
        kind: "literal",
        detectionMode: "matcher_only",
        severity: "high",
        remediation: "rotate_credential",
      },
      {
        ruleId: "test.prefix",
        kind: "prefix",
        detectionMode: "matcher_only",
        severity: "medium",
        remediation: null,
      },
      {
        ruleId: "test.suffix",
        kind: "suffix",
        detectionMode: "matcher_only",
        severity: "low",
        remediation: null,
      },
      {
        ruleId: "test.pattern",
        kind: "pattern",
        detectionMode: "matcher_only",
        severity: "critical",
        remediation: "remove_sensitive_value",
      },
    ]);
    assert.equal(
      scanner.rules.some((rule) => rule.ruleId === "github.classic-pat"),
      false,
    );

    const result = scanner.scan(customSource);
    assert.equal(result.sourceBytes, new TextEncoder().encode(customSource).length);
    assert.equal(result.findingCount, 4);
    assert.deepEqual(
      result.findings.map((finding) => ({
        ruleId: finding.ruleId,
        span: finding.span,
        line: finding.line,
        column: finding.column,
        severity: finding.severity,
        confidence: finding.confidence,
        remediation: finding.remediation,
      })),
      [
        {
          ruleId: "test.literal",
          span: { start: 0, end: 17 },
          line: 1,
          column: 1,
          severity: "high",
          confidence: "high",
          remediation: "rotate_credential",
        },
        {
          ruleId: "test.prefix",
          span: { start: 18, end: 26 },
          line: 2,
          column: 1,
          severity: "medium",
          confidence: "high",
          remediation: null,
        },
        {
          ruleId: "test.suffix",
          span: { start: 27, end: 36 },
          line: 3,
          column: 1,
          severity: "low",
          confidence: "high",
          remediation: null,
        },
        {
          ruleId: "test.pattern",
          span: { start: 37, end: 47 },
          line: 4,
          column: 1,
          severity: "critical",
          confidence: "high",
          remediation: "remove_sensitive_value",
        },
      ],
    );
    assert.equal(
      result.findings.some((finding) => finding.ruleId === "github.classic-pat"),
      false,
    );
    plainData(result);
    assert.equal(JSON.stringify(result).includes(customSource), false);
  } finally {
    scanner.dispose();
  }
});

test("built-ins and custom rules compile together", () => {
  const scanner = api.createScanner({
    rules: [customRules[0]],
  });
  try {
    assert.ok(
      scanner.rules.some((rule) => rule.ruleId === "github.classic-pat"),
    );
    assert.ok(
      scanner.rules.some((rule) => rule.ruleId === "test.literal"),
    );

    const source = `GITHUB_TOKEN=${githubToken}\ncustom-secret-123`;
    const result = scanner.scan(source);
    const ids = result.findings.map((finding) => finding.ruleId);
    assert.ok(ids.includes("github.classic-pat"));
    assert.ok(ids.includes("test.literal"));
    assert.deepEqual(ids, ["github.classic-pat", "test.literal"]);
    assert.deepEqual(
      result.findings[1].span,
      occurrenceSpan(source, "custom-secret-123"),
    );
  } finally {
    scanner.dispose();
  }
});

test("candidate detection is rule-independent and explanations are always present", () => {
  const scanner = api.createScanner({ builtins: "none" });
  try {
    const result = scanner.scan(candidateSource);
    assert.equal(result.findingCount, 0);
    assert.equal(result.candidateCount, 1);
    assert.equal(result.needsReview, true);
    assert.equal(result.hasCritical, false);
    assert.deepEqual(result.candidates[0], {
      kind: "recovery_like_code",
      evidence: "structural",
      span: { start: 0, end: 19 },
      line: 1,
      column: 1,
      explanation: { kind: "ambiguous", evidence: "structural" },
    });
  } finally {
    scanner.dispose();
  }
});

test("finding explanations are opt-in; candidate explanations are not", () => {
  const scanner = api.createScanner(customScannerOptions());
  try {
    const without = scanner.scan(customSource);
    assert.equal("explanation" in without.findings[0], false);

    const withExplanations = scanner.scan(customSource, {
      includeExplanations: true,
    });
    assert.deepEqual(withExplanations.findings[0].explanation, {
      kind: "classified",
      detectionMode: "matcher_only",
    });
    assert.equal(
      withExplanations.findings.every(
        (finding) => finding.explanation?.kind === "classified",
      ),
      true,
    );

    const candidate = scanner.scan(candidateSource);
    assert.deepEqual(candidate.candidates[0].explanation, {
      kind: "ambiguous",
      evidence: "structural",
    });
  } finally {
    scanner.dispose();
  }
});
