/** Synchronous scanner construction, scanning, and deterministic ownership. */

import { CribraError } from "./errors.js";
import {
  isRemediation,
  isSeverity,
  remediationToNative,
  severityToNative,
} from "./enums.js";
import type { Runtime } from "./core.js";
import {
  projectBatchEntry,
  projectRuleMetadata,
  projectScanResult,
  projectSummary,
} from "./result.js";
import type {
  BatchEntry,
  BatchResult,
  CribraErrorCode,
  RuleMetadata,
  RuleSpec,
  ScanOptions,
  ScanResult,
  ScanSource,
  Scanner,
  ScannerOptions,
  Severity,
} from "./types.js";
import type {
  FindingSeverity,
  RemediationKind,
  ScanEngine,
  ScanEngineBuilder,
  ScanBatchSource,
} from "./wasm/cribra.js";

interface NormalizedRule {
  readonly kind: RuleSpec["kind"];
  readonly id: string;
  readonly severity: Severity;
  readonly remediation: number | null;
  /** Literal/prefix/suffix value or pattern source. */
  readonly payload: string;
}

interface NormalizedScannerOptions {
  readonly builtins: "none" | "current";
  readonly financialBuiltins: boolean;
  readonly personalBuiltins: boolean;
  readonly rules: readonly NormalizedRule[];
}

function invalidArgument(): CribraError {
  return new CribraError("invalid_argument", { domain: "scanner" });
}

function safeFree(handle: { free(): void } | undefined): void {
  if (handle === undefined) return;
  try {
    handle.free();
  } catch {
    // A generated free() must never make disposal retryable or mask failure.
  }
}

function normalizeRule(rule: unknown): NormalizedRule {
  if (rule === null || typeof rule !== "object" || Array.isArray(rule)) {
    throw invalidArgument();
  }
  const candidate = rule as Record<string, unknown>;
  const kind = candidate.kind;
  if (
    kind !== "literal" &&
    kind !== "prefix" &&
    kind !== "suffix" &&
    kind !== "pattern"
  ) {
    throw invalidArgument();
  }
  if (typeof candidate.id !== "string") {
    throw invalidArgument();
  }
  if (!isSeverity(candidate.severity)) {
    throw invalidArgument();
  }

  let remediation: number | null = null;
  const remediationValue = candidate.remediation;
  if (remediationValue !== undefined && remediationValue !== null) {
    if (!isRemediation(remediationValue)) {
      throw invalidArgument();
    }
    remediation = remediationToNative(remediationValue);
  }

  if (kind === "pattern") {
    if (candidate.captureGroup !== undefined) {
      throw new CribraError("unsupported_capability", {
        domain: "capability",
      });
    }
    if (typeof candidate.pattern !== "string") {
      throw invalidArgument();
    }
    return {
      kind,
      id: candidate.id,
      severity: candidate.severity,
      remediation,
      payload: candidate.pattern,
    };
  }

  if (typeof candidate.value !== "string") {
    throw invalidArgument();
  }
  return {
    kind,
    id: candidate.id,
    severity: candidate.severity,
    remediation,
    payload: candidate.value,
  };
}

function normalizeScannerOptions(
  options: ScannerOptions | undefined,
): NormalizedScannerOptions {
  if (options === undefined) {
    return {
      builtins: "current",
      financialBuiltins: false,
      personalBuiltins: false,
      rules: [],
    };
  }
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    throw invalidArgument();
  }

  const builtins = options.builtins;
  if (builtins !== undefined && builtins !== "none" && builtins !== "current") {
    throw invalidArgument();
  }
  const financialBuiltins = options.financialBuiltins;
  if (
    financialBuiltins !== undefined &&
    typeof financialBuiltins !== "boolean"
  ) {
    throw invalidArgument();
  }
  const personalBuiltins = options.personalBuiltins;
  if (personalBuiltins !== undefined && typeof personalBuiltins !== "boolean") {
    throw invalidArgument();
  }

  const rules = options.rules;
  if (rules !== undefined && !Array.isArray(rules)) {
    throw invalidArgument();
  }

  return {
    builtins: builtins ?? "current",
    financialBuiltins: financialBuiltins ?? false,
    personalBuiltins: personalBuiltins ?? false,
    rules: (rules ?? []).map(normalizeRule),
  };
}

function normalizeScanOptions(options: ScanOptions | undefined): boolean {
  if (options === undefined) return false;
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    throw invalidArgument();
  }
  const includeExplanations = options.includeExplanations;
  if (includeExplanations === undefined) return false;
  if (typeof includeExplanations !== "boolean") {
    throw invalidArgument();
  }
  return includeExplanations;
}

function normalizeBatchSources(sources: readonly ScanSource[]): ScanBatchSource[] {
  if (!Array.isArray(sources)) {
    throw invalidArgument();
  }
  return sources.map((entry) => {
    if (entry === null || typeof entry !== "object") {
      throw invalidArgument();
    }
    const candidate = entry as Record<string, unknown>;
    if (
      typeof candidate.id !== "string" ||
      typeof candidate.source !== "string"
    ) {
      throw invalidArgument();
    }
    return { id: candidate.id, source: candidate.source };
  });
}

const BUILD_ERROR_CODES: readonly CribraErrorCode[] = [
  "invalid_pattern",
  "pattern_matches_empty",
  "missing_capture_group",
  "empty_rule_id",
  "duplicate_rule_id",
  "empty_matcher",
  "too_many_rules",
  "automaton_build",
  "invalid_remediation",
];

function normalizeBuildError(error: unknown): CribraError {
  if (error instanceof CribraError) return error;
  if (error instanceof Error) {
    const details = error as { code?: unknown; ruleId?: unknown };
    const code = details.code;
    if (
      typeof code === "number" &&
      Number.isInteger(code) &&
      code >= 0 &&
      code < BUILD_ERROR_CODES.length
    ) {
      return new CribraError(BUILD_ERROR_CODES[code] as CribraErrorCode, {
        domain: "scanner",
        ...(typeof details.ruleId === "string"
          ? { ruleId: details.ruleId }
          : {}),
      });
    }
  }
  return invalidArgument();
}

function applyRule(builder: ScanEngineBuilder, rule: NormalizedRule): void {
  const severity = severityToNative(rule.severity) as FindingSeverity;
  const remediation =
    rule.remediation === null
      ? undefined
      : (rule.remediation as RemediationKind);

  switch (rule.kind) {
    case "literal":
      if (remediation === undefined) {
        builder.addLiteral(rule.id, rule.payload, severity);
      } else {
        builder.addLiteralWithRemediation(
          rule.id,
          rule.payload,
          severity,
          remediation,
        );
      }
      return;
    case "prefix":
      if (remediation === undefined) {
        builder.addPrefix(rule.id, rule.payload, severity);
      } else {
        builder.addPrefixWithRemediation(
          rule.id,
          rule.payload,
          severity,
          remediation,
        );
      }
      return;
    case "suffix":
      if (remediation === undefined) {
        builder.addSuffix(rule.id, rule.payload, severity);
      } else {
        builder.addSuffixWithRemediation(
          rule.id,
          rule.payload,
          severity,
          remediation,
        );
      }
      return;
    case "pattern":
      if (remediation === undefined) {
        builder.addPattern(rule.id, rule.payload, severity);
      } else {
        builder.addPatternWithRemediation(
          rule.id,
          rule.payload,
          severity,
          remediation,
        );
      }
      return;
  }
}

function projectRuleList(engine: ScanEngine): readonly RuleMetadata[] {
  const count = engine.rulesCount();
  const views = engine.activeRules();
  const metadata: RuleMetadata[] = [];
  let failure: unknown;

  for (const view of views) {
    try {
      metadata.push(projectRuleMetadata(view));
    } catch (error) {
      failure ??= error;
    }
  }

  if (failure !== undefined || metadata.length !== count) {
    throw invalidArgument();
  }
  return Object.freeze(metadata);
}

export function createScanner(
  runtime: Runtime,
  options?: ScannerOptions,
): Scanner {
  const state = runtime.requireInitialized();
  const config = normalizeScannerOptions(options);

  let builder: ScanEngineBuilder | undefined;
  let engine: ScanEngine | undefined;
  let builderConsumed = false;

  try {
    builder = new state.wasm.ScanEngineBuilder(config.builtins === "current");
    if (config.financialBuiltins) {
      builder.addFinancialBuiltins();
    }
    if (config.personalBuiltins) {
      builder.addPersonalBuiltins();
    }
    for (const rule of config.rules) {
      applyRule(builder, rule);
    }

    // build() consumes the builder on success and on failure.
    builderConsumed = true;
    engine = builder.build();

    const rules = projectRuleList(engine);
    const rulesCount = rules.length;
    const activeEngine = engine;

    let disposed = false;

    function requireActive(): ScanEngine {
      if (disposed) {
        throw new CribraError("disposed", { domain: "scanner" });
      }
      return activeEngine;
    }

    return Object.freeze({
      get rulesCount() {
        return rulesCount;
      },
      get rules() {
        return rules;
      },
      get disposed() {
        return disposed;
      },
      scan(source: string, scanOptions?: ScanOptions): ScanResult {
        const nativeEngine = requireActive();
        if (typeof source !== "string") {
          throw invalidArgument();
        }
        const includeExplanations = normalizeScanOptions(scanOptions);

        let native: import("./wasm/cribra.js").ScanResult;
        try {
          native = nativeEngine.scan(source);
        } catch {
          throw invalidArgument();
        }
        try {
          return projectScanResult(nativeEngine, native, includeExplanations);
        } finally {
          safeFree(native);
        }
      },
      scanBatch(
        sources: readonly ScanSource[],
        scanOptions?: ScanOptions,
      ): BatchResult {
        const nativeEngine = requireActive();
        const batchSources = normalizeBatchSources(sources);
        const includeExplanations = normalizeScanOptions(scanOptions);

        let batch: import("./wasm/cribra.js").ScanBatchResult;
        try {
          batch = nativeEngine.scanBatch(batchSources);
        } catch {
          throw invalidArgument();
        }

        try {
          const length = batch.length;
          if (!Number.isSafeInteger(length) || length < 0) {
            throw invalidArgument();
          }
          const entries: BatchEntry[] = [];
          for (let index = 0; index < length; index += 1) {
            const entry = batch.entryAt(index);
            try {
              const id = entry.id;
              const sourceBytes = entry.sourceBytes;
              const nativeResult = entry.result;
              try {
                entries.push(
                  projectBatchEntry(
                    id,
                    sourceBytes,
                    projectScanResult(
                      nativeEngine,
                      nativeResult,
                      includeExplanations,
                    ),
                  ),
                );
              } finally {
                safeFree(nativeResult);
              }
            } finally {
              safeFree(entry);
            }
          }

          const summaryView = batch.summary();
          const summary = projectSummary(summaryView);
          return Object.freeze({
            entries: Object.freeze(entries),
            summary,
          });
        } finally {
          safeFree(batch);
        }
      },
      dispose() {
        if (disposed) return;
        disposed = true;
        safeFree(activeEngine);
      },
    });
  } catch (error) {
    if (!builderConsumed) {
      safeFree(builder);
    }
    safeFree(engine);
    throw normalizeBuildError(error);
  }
}
