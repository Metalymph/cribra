/**
 * Public error model for the React Native backend.
 *
 * Native-independent: this module must never import the TurboModule, so the
 * pure Unicode offset utilities can be tested without a native runtime.
 *
 * Every message is a fixed constant. Nothing derived from scanned source,
 * matcher payloads, rule identifiers, or offsets is ever interpolated.
 */

export type CribraErrorCode =
  | 'disposed'
  | 'invalid_argument'
  | 'invalid_input_string'
  | 'invalid_utf8'
  | 'invalid_unicode_string'
  | 'invalid_span_value'
  | 'invalid_span_order'
  | 'span_out_of_bounds'
  | 'span_not_utf8_boundary'
  | 'span_not_utf16_boundary'
  | 'scanner_not_found'
  | 'scanner_create_failed'
  | 'abi_mismatch'
  | 'invalid_native_report'
  | 'invalid_pattern'
  | 'pattern_matches_empty'
  | 'missing_capture_group'
  | 'empty_rule_id'
  | 'duplicate_rule_id'
  | 'empty_matcher'
  | 'too_many_rules'
  | 'automaton_build'
  | 'native_error';

const MESSAGES: Record<CribraErrorCode, string> = {
  disposed: 'The Cribra scanner has been disposed.',
  invalid_argument: 'A Cribra argument is invalid.',
  invalid_input_string: 'The source could not be converted to UTF-8.',
  invalid_utf8: 'The source is not valid UTF-8.',
  invalid_unicode_string: 'The source contains an unpaired UTF-16 surrogate.',
  invalid_span_value: 'A span endpoint is not a safe non-negative integer.',
  invalid_span_order: 'The span start is after its end.',
  span_out_of_bounds: 'The span end exceeds the source length.',
  span_not_utf8_boundary: 'A span endpoint is not a UTF-8 scalar boundary.',
  span_not_utf16_boundary: 'A range endpoint is not a UTF-16 scalar boundary.',
  scanner_not_found: 'The native scanner handle is not available.',
  scanner_create_failed: 'The native scanner could not be created.',
  abi_mismatch: 'The linked Cribra native ABI is incompatible with this package.',
  invalid_native_report: 'The native scanner returned an unsupported value.',
  invalid_pattern: 'A custom rule pattern is invalid.',
  pattern_matches_empty: 'A custom rule pattern can match empty input.',
  missing_capture_group: 'A custom rule capture configuration is invalid.',
  empty_rule_id: 'A custom rule identifier is empty.',
  duplicate_rule_id: 'A rule identifier is duplicated.',
  empty_matcher: 'A custom rule matcher is empty.',
  too_many_rules: 'The configured rule count exceeds the scanner limit.',
  automaton_build: 'The scanner matcher could not be compiled.',
  native_error: 'The Cribra native operation failed.',
};

/**
 * Structured error with a fixed, source-free message.
 *
 * `detail` carries only privacy-safe diagnostic metadata returned by the C ABI
 * or a static internal field name. Offset failures never set it.
 */
export class CribraError extends Error {
  readonly code: CribraErrorCode;
  readonly detail?: string;
  /**
   * Caller-supplied rule identity for a construction failure, or a public
   * built-in ID for a catalog collision. Never embedded in the message and
   * never derived from scanned source or matcher contents.
   */
  readonly ruleId?: string;

  constructor(code: CribraErrorCode, detail?: string, ruleId?: string) {
    super(MESSAGES[code]);
    this.name = 'CribraError';
    this.code = code;
    if (detail !== undefined && detail.length > 0) {
      this.detail = detail;
    }
    if (ruleId !== undefined && ruleId.length > 0) {
      this.ruleId = ruleId;
    }
  }
}
