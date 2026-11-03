/** Public structured error. Fixed messages never interpolate caller data. */

import type { ErrorDomain, CribraErrorCode } from "./types.js";

const MESSAGES: Partial<Record<CribraErrorCode, string>> = {
  not_initialized: "Cribra has not been initialized.",
  disposed: "The Cribra scanner has been disposed.",
  unsupported_capability: "The initialized backend does not support this operation.",
  invalid_argument: "A Cribra argument is invalid.",
  invalid_unicode_string: "The source contains an unpaired UTF-16 surrogate.",
  invalid_span_value: "Span endpoints must be nonnegative safe integers.",
  invalid_span_order: "The span start must not exceed the span end.",
  span_out_of_bounds: "The span exceeds the UTF-8 byte length of the source.",
  span_not_utf8_boundary: "A span endpoint is not a UTF-8 scalar boundary.",
  invalid_pattern: "The scanner rejected a rule pattern.",
  pattern_matches_empty: "A rule pattern can match the empty string.",
  missing_capture_group: "A capture-aware rule names an absent capture group.",
  empty_rule_id: "A rule ID is empty.",
  duplicate_rule_id: "A rule ID is duplicated or collides with a built-in rule.",
  empty_matcher: "A literal, prefix, or suffix matcher is empty.",
  too_many_rules: "The scanner rule capacity was exceeded.",
  automaton_build: "The scanner automaton could not be compiled.",
  invalid_remediation: "The configured remediation is not valid for an input rule.",
  wasm_load_failed: "The Cribra WebAssembly asset could not be loaded.",
  wasm_instantiate_failed: "The Cribra WebAssembly module could not be initialized.",
};

function defaultDomain(code: CribraErrorCode): ErrorDomain {
  if (code.startsWith("stream_")) return "stream";
  if (code.startsWith("incremental_")) return "incremental";
  if (code.startsWith("transform_")) return "transform";
  if (code.startsWith("share_")) return "share";
  switch (code) {
    case "not_initialized":
    case "wasm_load_failed":
    case "wasm_instantiate_failed":
      return "init";
    case "unsupported_capability":
      return "capability";
    case "invalid_unicode_string":
    case "invalid_span_value":
    case "invalid_span_order":
    case "span_out_of_bounds":
    case "span_not_utf8_boundary":
      return "offset";
    default:
      return "scanner";
  }
}

function defaultMessage(code: CribraErrorCode): string {
  return MESSAGES[code] ?? `Cribra error: ${code}.`;
}

export interface CribraErrorOptions {
  /** Overrides the code-derived domain; used for overloaded codes. */
  readonly domain?: ErrorDomain;
  /** Stable metadata; never source text or matched values. */
  readonly ruleId?: string;
  /** Fixed diagnostic override. Must not interpolate caller data. */
  readonly message?: string;
}

export class CribraError extends Error {
  readonly code: CribraErrorCode;
  readonly domain: ErrorDomain;
  readonly ruleId?: string;

  constructor(code: CribraErrorCode, options?: CribraErrorOptions) {
    super(options?.message ?? defaultMessage(code));
    this.name = "CribraError";
    this.code = code;
    this.domain = options?.domain ?? defaultDomain(code);
    if (options?.ruleId !== undefined) {
      this.ruleId = options.ruleId;
    }
  }
}

export function isCribraError(value: unknown): value is CribraError {
  return value instanceof CribraError;
}

/** Normalizes any non-Cribra failure without exposing its raw diagnostic. */
export function normalizeUnknownError(
  error: unknown,
  code: CribraErrorCode,
  domain?: ErrorDomain,
  ruleId?: string,
): CribraError {
  if (error instanceof CribraError) {
    return error;
  }
  return new CribraError(code, {
    ...(domain === undefined ? {} : { domain }),
    ...(ruleId === undefined ? {} : { ruleId }),
  });
}
