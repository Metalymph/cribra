//! Core-derived categories on the existing garbage-collected JavaScript Error.
use wasm_bindgen::prelude::*;

/// Stable scanner-construction categories. Diagnostics are never machine codes.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScannerBuildErrorCode {
    /// Core rejected regex syntax or compilation resources.
    InvalidPattern = 0,
    /// Core rejected a zero-length-capable regex.
    PatternMatchesEmpty = 1,
    /// Core's capture-aware rule names an absent group.
    MissingCaptureGroup = 2,
    /// Core rejected an empty rule ID.
    EmptyRuleId = 3,
    /// Core rejected duplicate rule IDs or a catalog collision.
    DuplicateRuleId = 4,
    /// Core rejected an empty literal, prefix or suffix.
    EmptyMatcher = 5,
    /// Core's rule-index capacity was exceeded.
    TooManyRules = 6,
    /// Core could not compile its shared automaton.
    AutomatonBuild = 7,
    /// The output-only Unknown remediation has no Core configuration value.
    InvalidRemediation = 8,
}

#[wasm_bindgen(typescript_custom_section)]
const BUILD_ERROR_TYPES: &str = r#"
/** Enriched native JavaScript Error, garbage-collected; no free() is needed.
 * Core regex diagnostics may contain configuration patterns, never scanned source.
 */
export interface ScannerBuildError extends Error {
    readonly code: ScannerBuildErrorCode;
    readonly ruleId: string | undefined;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "ScannerBuildError")]
    type ErrorProperties;
    #[wasm_bindgen(method, setter, js_name = code)]
    fn set_code(this: &ErrorProperties, value: ScannerBuildErrorCode);
    #[wasm_bindgen(method, setter, js_name = ruleId)]
    fn set_rule_id(this: &ErrorProperties, value: Option<String>);
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BuildFailure {
    pub(crate) code: ScannerBuildErrorCode,
    pub(crate) rule_id: Option<String>,
    pub(crate) message: String,
}

impl BuildFailure {
    pub(crate) fn rule(error: cribra::RuleError, id: Option<String>) -> Self {
        let code = match &error {
            cribra::RuleError::InvalidPattern(_) => ScannerBuildErrorCode::InvalidPattern,
            cribra::RuleError::PatternMatchesEmpty => ScannerBuildErrorCode::PatternMatchesEmpty,
            cribra::RuleError::MissingCaptureGroup { .. } => {
                ScannerBuildErrorCode::MissingCaptureGroup
            }
        };
        Self {
            code,
            rule_id: id,
            message: error.to_string(),
        }
    }

    pub(crate) fn scanner(error: cribra::ScannerBuildError) -> Self {
        use cribra::ScannerBuildError as Core;
        let message = error.to_string();
        let (code, rule_id) = match error {
            Core::Rule(error) => {
                let mut failure = Self::rule(error, None);
                failure.message = message;
                return failure;
            }
            Core::EmptyRuleId => (ScannerBuildErrorCode::EmptyRuleId, None),
            Core::DuplicateRuleId { rule_id } => (
                ScannerBuildErrorCode::DuplicateRuleId,
                Some(rule_id.to_string()),
            ),
            Core::EmptyMatcher { rule_id } => (
                ScannerBuildErrorCode::EmptyMatcher,
                Some(rule_id.to_string()),
            ),
            Core::TooManyRules => (ScannerBuildErrorCode::TooManyRules, None),
            Core::AutomatonBuild(_) => (ScannerBuildErrorCode::AutomatonBuild, None),
        };
        Self {
            code,
            rule_id,
            message,
        }
    }

    pub(crate) fn into_js(self) -> JsError {
        let error = JsError::new(&self.message);
        let value: JsValue = error.clone().into();
        let properties: &ErrorProperties = value.unchecked_ref();
        properties.set_code(self.code);
        properties.set_rule_id(self.rule_id);
        error
    }
}
