//! Privacy-safe validation of tool arguments.
//!
//! Validation is deliberately manual and operates on `serde_json::Value`.
//! Deserialization error text is never propagated because it can echo
//! client-supplied keys or values. Every failure maps to one fixed,
//! source-free [`ToolError`].
//!
//! The published JSON schemas remain the client-facing contract. This module
//! enforces the runtime constraints that a schema alone cannot express:
//! UTF-8 byte limits, regex compilation, custom-rule identity collisions and
//! mode-specific option restrictions.

use serde_json::{Map, Value};

use cribra::Severity;

use crate::limits::{
    MAX_CUSTOM_PATTERN_CHARS, MAX_CUSTOM_RULE_ID_BYTES, MAX_CUSTOM_RULES, MAX_REDACTION_CHARS,
    MAX_SOURCE_BYTES, MAX_TEMPLATE_NAMESPACE_CHARS,
};

/// Stable tool error codes declared by the advertised output schemas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    /// Arguments do not satisfy the published input contract.
    InvalidArguments,
    /// Custom rules cannot be combined into a scanner.
    InvalidRuleConfiguration,
    /// A custom regular expression is invalid or can match empty input.
    InvalidRulePattern,
    /// The requested transform mode is not advertised.
    UnsupportedMode,
    /// Template transformation cannot apply overlapping findings.
    OverlappingSpans,
    /// A finding or candidate limit would be exceeded.
    ResultLimitExceeded,
    /// The encoded JSON-RPC response would exceed the transport limit.
    ResponseTooLarge,
    /// An unexpected internal failure occurred.
    InternalError,
}

impl ErrorCode {
    /// Returns the stable wire value for this error code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArguments => "invalid_arguments",
            Self::InvalidRuleConfiguration => "invalid_rule_configuration",
            Self::InvalidRulePattern => "invalid_rule_pattern",
            Self::UnsupportedMode => "unsupported_mode",
            Self::OverlappingSpans => "overlapping_spans",
            Self::ResultLimitExceeded => "result_limit_exceeded",
            Self::ResponseTooLarge => "response_too_large",
            Self::InternalError => "internal_error",
        }
    }

    /// Returns the fixed, bounded, source-free message for this code.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidArguments => "Tool arguments do not satisfy the published input schema.",
            Self::InvalidRuleConfiguration => "Custom rule configuration is invalid.",
            Self::InvalidRulePattern => {
                "A custom rule pattern is invalid or can match an empty string."
            }
            Self::UnsupportedMode => "The requested transform mode is not supported.",
            Self::OverlappingSpans => {
                "Template transformation does not support overlapping findings."
            }
            Self::ResultLimitExceeded => "The result exceeds the 4096-item limit.",
            Self::ResponseTooLarge => {
                "The serialized tool result exceeds the 8 MiB response limit."
            }
            Self::InternalError => "Tool execution failed.",
        }
    }
}

/// One sanitized tool failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolError {
    code: ErrorCode,
}

impl ToolError {
    /// Creates a failure with a fixed code and message.
    #[must_use]
    pub const fn new(code: ErrorCode) -> Self {
        Self { code }
    }

    /// Returns the stable wire code.
    #[must_use]
    pub const fn code(self) -> ErrorCode {
        self.code
    }
}

/// A validated custom regular-expression rule.
#[derive(Clone, Debug)]
pub struct CustomRule {
    /// Stable rule identifier.
    pub id: String,
    /// Regular-expression source.
    pub pattern: String,
    /// Severity assigned to findings.
    pub severity: Severity,
}

/// Validated `cribra_scan` arguments.
#[derive(Clone, Debug)]
pub struct ScanArgs {
    /// Source text to scan.
    pub source: String,
    /// Optional custom rules added to the built-in catalog.
    pub rules: Vec<CustomRule>,
}

/// One of the advertised non-keyed transform modes.
#[derive(Clone, Debug)]
pub enum TransformMode {
    /// Replace findings with replacement text.
    Redact {
        /// Optional replacement; `None` uses Core's `[REDACTED]`.
        replacement: Option<String>,
    },
    /// Replace findings with semantic placeholders.
    Template {
        /// Placeholder namespace.
        namespace: String,
        /// Whether repeated findings receive occurrence numbers.
        numbered: bool,
    },
}

impl TransformMode {
    /// Returns the wire mode name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Redact { .. } => "redact",
            Self::Template { .. } => "template",
        }
    }
}

/// Validated `cribra_transform` arguments.
#[derive(Clone, Debug)]
pub struct TransformArgs {
    /// Source text to scan and transform.
    pub source: String,
    /// Optional custom rules added to the built-in catalog.
    pub rules: Vec<CustomRule>,
    /// Validated transform mode and mode-specific options.
    pub mode: TransformMode,
}

/// Parses and validates `cribra_scan` arguments.
pub fn parse_scan(arguments: &Value) -> Result<ScanArgs, ToolError> {
    let object = arguments
        .as_object()
        .ok_or(ToolError::new(ErrorCode::InvalidArguments))?;
    reject_unknown_keys(object, &["source", "rules"])?;

    let source = required_string(object, "source")?;
    if source.len() > MAX_SOURCE_BYTES {
        return Err(ToolError::new(ErrorCode::InvalidArguments));
    }

    Ok(ScanArgs {
        source: source.to_owned(),
        rules: parse_rules(object.get("rules"))?,
    })
}

/// Parses and validates `cribra_transform` arguments.
pub fn parse_transform(arguments: &Value) -> Result<TransformArgs, ToolError> {
    let object = arguments
        .as_object()
        .ok_or(ToolError::new(ErrorCode::InvalidArguments))?;
    reject_unknown_keys(object, &["source", "rules", "mode", "options"])?;

    let source = required_string(object, "source")?;
    if source.len() > MAX_SOURCE_BYTES {
        return Err(ToolError::new(ErrorCode::InvalidArguments));
    }

    let mode_name = required_string(object, "mode")?;
    let mode = match mode_name {
        "redact" => parse_redact_mode(object.get("options"))?,
        "template" => parse_template_mode(object.get("options"))?,
        _ => return Err(ToolError::new(ErrorCode::UnsupportedMode)),
    };

    Ok(TransformArgs {
        source: source.to_owned(),
        rules: parse_rules(object.get("rules"))?,
        mode,
    })
}

fn parse_redact_mode(options: Option<&Value>) -> Result<TransformMode, ToolError> {
    let Some(options) = options else {
        return Ok(TransformMode::Redact { replacement: None });
    };
    let object = options
        .as_object()
        .ok_or(ToolError::new(ErrorCode::InvalidArguments))?;
    reject_unknown_keys(object, &["replacement"])?;

    let replacement = match object.get("replacement") {
        None => None,
        Some(value) => {
            let text = value
                .as_str()
                .ok_or(ToolError::new(ErrorCode::InvalidArguments))?;
            let length = text.chars().count();
            if length == 0 || length > MAX_REDACTION_CHARS {
                return Err(ToolError::new(ErrorCode::InvalidArguments));
            }
            Some(text.to_owned())
        }
    };

    Ok(TransformMode::Redact { replacement })
}

fn parse_template_mode(options: Option<&Value>) -> Result<TransformMode, ToolError> {
    let namespace_default = "CRIBRA";
    let Some(options) = options else {
        return Ok(TransformMode::Template {
            namespace: namespace_default.to_owned(),
            numbered: false,
        });
    };
    let object = options
        .as_object()
        .ok_or(ToolError::new(ErrorCode::InvalidArguments))?;
    reject_unknown_keys(object, &["namespace", "numbered"])?;

    let namespace = match object.get("namespace") {
        None => namespace_default.to_owned(),
        Some(value) => {
            let text = value
                .as_str()
                .ok_or(ToolError::new(ErrorCode::InvalidArguments))?;
            validate_identifier_chars(text, MAX_TEMPLATE_NAMESPACE_CHARS)?;
            text.to_owned()
        }
    };

    let numbered = match object.get("numbered") {
        None => false,
        Some(value) => value
            .as_bool()
            .ok_or(ToolError::new(ErrorCode::InvalidArguments))?,
    };

    Ok(TransformMode::Template {
        namespace,
        numbered,
    })
}

fn parse_rules(value: Option<&Value>) -> Result<Vec<CustomRule>, ToolError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let rules = value
        .as_array()
        .ok_or(ToolError::new(ErrorCode::InvalidArguments))?;
    if rules.len() > MAX_CUSTOM_RULES {
        return Err(ToolError::new(ErrorCode::InvalidArguments));
    }

    let mut parsed = Vec::with_capacity(rules.len());
    for rule in rules {
        let object = rule
            .as_object()
            .ok_or(ToolError::new(ErrorCode::InvalidArguments))?;
        reject_unknown_keys(object, &["id", "pattern", "severity"])?;

        let id = required_string(object, "id")?;
        validate_identifier_chars(id, MAX_CUSTOM_RULE_ID_BYTES)?;

        let pattern = required_string(object, "pattern")?;
        let pattern_chars = pattern.chars().count();
        if pattern_chars == 0 || pattern_chars > MAX_CUSTOM_PATTERN_CHARS {
            return Err(ToolError::new(ErrorCode::InvalidArguments));
        }

        let severity_name = required_string(object, "severity")?;
        let severity = match severity_name {
            "info" => Severity::Info,
            "low" => Severity::Low,
            "medium" => Severity::Medium,
            "high" => Severity::High,
            "critical" => Severity::Critical,
            _ => return Err(ToolError::new(ErrorCode::InvalidArguments)),
        };

        parsed.push(CustomRule {
            id: id.to_owned(),
            pattern: pattern.to_owned(),
            severity,
        });
    }

    Ok(parsed)
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, ToolError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(ToolError::new(ErrorCode::InvalidArguments))
}

fn reject_unknown_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), ToolError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(ToolError::new(ErrorCode::InvalidArguments));
    }
    Ok(())
}

fn validate_identifier_chars(value: &str, max_chars: usize) -> Result<(), ToolError> {
    let length = value.chars().count();
    if length == 0 || length > max_chars {
        return Err(ToolError::new(ErrorCode::InvalidArguments));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ToolError::new(ErrorCode::InvalidArguments));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::MAX_SOURCE_BYTES;

    fn scan_error(arguments: Value) -> ErrorCode {
        parse_scan(&arguments)
            .expect_err("arguments must be rejected")
            .code()
    }

    fn transform_error(arguments: Value) -> ErrorCode {
        parse_transform(&arguments)
            .expect_err("arguments must be rejected")
            .code()
    }

    fn rule(id: &str, pattern: &str) -> Value {
        serde_json::json!({
            "id": id,
            "pattern": pattern,
            "severity": "high",
        })
    }

    #[test]
    fn scan_accepts_minimal_and_custom_rule_shapes() {
        let parsed = parse_scan(&serde_json::json!({
            "source": "hello",
            "rules": [rule("custom.token", "SECRET[0-9]+")],
        }))
        .expect("valid scan");
        assert_eq!(parsed.source, "hello");
        assert_eq!(parsed.rules.len(), 1);
        assert_eq!(parsed.rules[0].id, "custom.token");
        assert_eq!(parsed.rules[0].pattern, "SECRET[0-9]+");
        assert_eq!(parsed.rules[0].severity, Severity::High);

        let empty = parse_scan(&serde_json::json!({"source": "hello"})).expect("no rules");
        assert!(empty.rules.is_empty());
    }

    #[test]
    fn scan_rejects_shape_type_and_field_violations() {
        assert_eq!(
            scan_error(serde_json::json!("not an object")),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            scan_error(serde_json::json!({})),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            scan_error(serde_json::json!({"source": 1})),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            scan_error(serde_json::json!({"source": "x", "extra": true})),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            scan_error(serde_json::json!({"source": "x", "rules": {}})),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            scan_error(
                serde_json::json!({"source": "x", "rules": [rule("r", "a"), {"id": "r2", "pattern": "b"}]})
            ),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            scan_error(
                serde_json::json!({"source": "x", "rules": [rule("r", "a"), {"id": "r2", "pattern": "b", "severity": "high", "extra": 1}]})
            ),
            ErrorCode::InvalidArguments
        );
    }

    #[test]
    fn scan_enforces_source_and_rule_limits_by_bytes_and_characters() {
        let exactly_max = "a".repeat(MAX_SOURCE_BYTES);
        assert!(parse_scan(&serde_json::json!({"source": exactly_max})).is_ok());
        let over_max = "a".repeat(MAX_SOURCE_BYTES + 1);
        assert_eq!(
            scan_error(serde_json::json!({"source": over_max})),
            ErrorCode::InvalidArguments
        );

        let too_many: Vec<Value> = (0..=MAX_CUSTOM_RULES)
            .map(|index| rule(&format!("r{index}"), "a"))
            .collect();
        assert_eq!(
            scan_error(serde_json::json!({"source": "x", "rules": too_many})),
            ErrorCode::InvalidArguments
        );

        assert_eq!(
            scan_error(serde_json::json!({"source": "x", "rules": [rule("", "a")]})),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            scan_error(serde_json::json!({"source": "x", "rules": [rule("bad id", "a")]})),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            scan_error(
                serde_json::json!({"source": "x", "rules": [rule(&"a".repeat(MAX_CUSTOM_RULE_ID_BYTES + 1), "a")]})
            ),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            scan_error(serde_json::json!({"source": "x", "rules": [rule("r", "")]})),
            ErrorCode::InvalidArguments
        );
        let long_pattern = "a".repeat(MAX_CUSTOM_PATTERN_CHARS + 1);
        assert_eq!(
            scan_error(serde_json::json!({"source": "x", "rules": [rule("r", &long_pattern)]})),
            ErrorCode::InvalidArguments
        );
        let exact_pattern = "a".repeat(MAX_CUSTOM_PATTERN_CHARS);
        assert!(
            parse_scan(&serde_json::json!({"source": "x", "rules": [rule("r", &exact_pattern)]}))
                .is_ok()
        );
    }

    #[test]
    fn scan_rejects_unknown_severities() {
        for severity in ["", "INFO", "severe", "critical "] {
            assert_eq!(
                scan_error(serde_json::json!({
                    "source": "x",
                    "rules": [{"id": "r", "pattern": "a", "severity": severity}],
                })),
                ErrorCode::InvalidArguments
            );
        }
    }

    #[test]
    fn transform_accepts_both_advertised_modes_and_defaults() {
        let redact = parse_transform(&serde_json::json!({"source": "x", "mode": "redact"}))
            .expect("default redact");
        assert!(matches!(
            redact.mode,
            TransformMode::Redact { replacement: None }
        ));

        let custom = parse_transform(&serde_json::json!({
            "source": "x",
            "mode": "redact",
            "options": {"replacement": "***"},
        }))
        .expect("custom redact");
        assert!(matches!(
            custom.mode,
            TransformMode::Redact {
                replacement: Some(ref value)
            } if value == "***"
        ));

        let template = parse_transform(&serde_json::json!({"source": "x", "mode": "template"}))
            .expect("default template");
        assert!(matches!(
            template.mode,
            TransformMode::Template {
                ref namespace,
                numbered: false,
            } if namespace == "CRIBRA"
        ));

        let custom = parse_transform(&serde_json::json!({
            "source": "x",
            "mode": "template",
            "options": {"namespace": "MY_NS", "numbered": true},
        }))
        .expect("custom template");
        assert!(matches!(
            custom.mode,
            TransformMode::Template {
                ref namespace,
                numbered: true,
            } if namespace == "MY_NS"
        ));
    }

    #[test]
    fn transform_rejects_unsupported_modes_and_keyed_options() {
        assert_eq!(
            transform_error(serde_json::json!({"source": "x", "mode": "synthesize"})),
            ErrorCode::UnsupportedMode
        );
        assert_eq!(
            transform_error(serde_json::json!({"source": "x", "mode": "pseudonymize"})),
            ErrorCode::UnsupportedMode
        );
        assert_eq!(
            transform_error(serde_json::json!({"source": "x", "mode": "redact", "key": "secret"})),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            transform_error(serde_json::json!({
                "source": "x",
                "mode": "redact",
                "options": {"key": "secret"},
            })),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            transform_error(serde_json::json!({
                "source": "x",
                "mode": "template",
                "options": {"namespace": "NS", "numbered": false, "key": "secret"},
            })),
            ErrorCode::InvalidArguments
        );
    }

    #[test]
    fn transform_validates_mode_options() {
        assert_eq!(
            transform_error(
                serde_json::json!({"source": "x", "mode": "redact", "options": "text"})
            ),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            transform_error(serde_json::json!({
                "source": "x",
                "mode": "redact",
                "options": {"replacement": ""},
            })),
            ErrorCode::InvalidArguments
        );
        let too_long = "a".repeat(MAX_REDACTION_CHARS + 1);
        assert_eq!(
            transform_error(serde_json::json!({
                "source": "x",
                "mode": "redact",
                "options": {"replacement": too_long},
            })),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            transform_error(serde_json::json!({
                "source": "x",
                "mode": "template",
                "options": {"namespace": ""},
            })),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            transform_error(serde_json::json!({
                "source": "x",
                "mode": "template",
                "options": {"namespace": "bad namespace"},
            })),
            ErrorCode::InvalidArguments
        );
        let too_long = "a".repeat(MAX_TEMPLATE_NAMESPACE_CHARS + 1);
        assert_eq!(
            transform_error(serde_json::json!({
                "source": "x",
                "mode": "template",
                "options": {"namespace": too_long},
            })),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            transform_error(serde_json::json!({
                "source": "x",
                "mode": "template",
                "options": {"numbered": "yes"},
            })),
            ErrorCode::InvalidArguments
        );
        assert_eq!(
            transform_error(serde_json::json!({
                "source": "x",
                "mode": "template",
                "options": {"namespace": "NS", "numbered": true, "extra": 1},
            })),
            ErrorCode::InvalidArguments
        );
    }

    #[test]
    fn transform_accepts_rule_shapes_and_defers_regex_compilation() {
        // Pattern syntax is validated by Core, not by the argument parser, so
        // a malformed regex reaches the adapter and maps to invalid_rule_pattern.
        let parsed = parse_transform(&serde_json::json!({
            "source": "x",
            "mode": "redact",
            "rules": [rule("r", "[")],
        }))
        .expect("shape-only rule validation");
        assert_eq!(parsed.rules.len(), 1);

        assert_eq!(
            transform_error(serde_json::json!({
                "source": "x",
                "mode": "template",
                "rules": [{"id": "bad id", "pattern": "abc", "severity": "high"}],
            })),
            ErrorCode::InvalidArguments
        );
        let parsed = parse_transform(&serde_json::json!({
            "source": "x",
            "mode": "template",
            "rules": [rule("custom.rule", "abc")],
        }))
        .expect("custom rule accepted for transform");
        assert_eq!(parsed.rules.len(), 1);
    }

    #[test]
    fn error_messages_are_fixed_and_bounded() {
        for code in [
            ErrorCode::InvalidArguments,
            ErrorCode::InvalidRuleConfiguration,
            ErrorCode::InvalidRulePattern,
            ErrorCode::UnsupportedMode,
            ErrorCode::OverlappingSpans,
            ErrorCode::ResultLimitExceeded,
            ErrorCode::ResponseTooLarge,
            ErrorCode::InternalError,
        ] {
            assert!(!code.as_str().is_empty());
            assert!(!code.message().is_empty());
            assert!(code.message().len() <= 256);
        }
    }
}
