//! Owned configuration, metadata and construction-error projections.

use std::{error::Error, fmt};

use crate::{DetectionMode, Remediation, RuleId, Severity};

/// Explicit rule selection; custom-only never implicitly adds built-ins.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ScannerConfig {
    /// The complete current built-in catalog, as used by `Scanner()`.
    BuiltInOnly,
    /// Only these custom rules, including an empty collection if requested.
    CustomOnly {
        /// Custom rules in caller order.
        rules: Vec<RuleSpec>,
    },
    /// These custom rules plus the complete current built-in catalog.
    BuiltInAndCustom {
        /// Custom rules in caller order; Core places these before built-ins.
        rules: Vec<RuleSpec>,
    },
}

/// Core matching families; matcher interpretation belongs exclusively to Core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RuleKind {
    /// Exact occurrences, without imposing token boundaries.
    Literal,
    /// Complete Core tokens beginning with the configured prefix.
    Prefix,
    /// Complete Core tokens ending with the configured suffix.
    Suffix,
    /// Non-overlapping, non-empty full regex matches supported by Core.
    Pattern,
}

/// An owned custom rule passed to the corresponding public Core constructor.
///
/// No adapter validation, normalization, built-in validator or regex engine is
/// applied. Custom rules have Core's matcher-authoritative semantics.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RuleSpec {
    /// Stable rule identity, validated by Core during scanner construction.
    pub id: RuleId,
    /// Which Core rule constructor to call.
    pub kind: RuleKind,
    /// Literal, prefix, suffix or pattern payload, consumed only as config input.
    pub value: String,
    /// Severity assigned to accepted findings.
    pub severity: Severity,
    /// Optional Core remediation; does not change detection behavior.
    pub remediation: Option<Remediation>,
}

/// Presentation-safe remediation text projected from Core.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RemediationMetadata {
    /// Core's action label.
    pub label: String,
    /// Core's remediation guidance.
    pub message: String,
}

/// Active rule facts in Core order, with no regex, matcher or source text.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RuleMetadata {
    /// Stable rule identity.
    pub id: RuleId,
    /// Matching family.
    pub kind: RuleKind,
    /// Core's validation mode.
    pub detection_mode: DetectionMode,
    /// Default rule severity.
    pub severity: Severity,
    /// Core-provided remediation text, if any.
    pub remediation: Option<RemediationMetadata>,
}

/// Stable symbolic error codes; numeric discriminants are not an API contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BuildErrorCode {
    /// Core rejected a regex (syntax or regex compilation resource limit).
    InvalidPattern,
    /// Core rejected a regex that can match zero bytes.
    PatternMatchesEmpty,
    /// Core's internal capture-aware rule refers to a missing capture group.
    MissingCaptureGroup,
    /// Core rejected an empty rule ID.
    EmptyRuleId,
    /// Core rejected a duplicate rule ID, including a built-in collision.
    DuplicateRuleId,
    /// Core rejected an empty literal, prefix or suffix matcher.
    EmptyMatcher,
    /// Core's compact rule-index capacity was exceeded.
    TooManyRules,
    /// Core could not compile its shared multi-pattern automaton.
    AutomatonBuild,
}

/// Structured construction failure, preserving the Core's diagnostic detail.
///
/// Code names are stable; human-readable Core diagnostics are not. Regex
/// diagnostics may include configuration payloads. No scanned source is used.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum ScannerBuildError {
    /// A Core rule constructor or scanner builder rejected the configuration.
    Rejected {
        /// Stable symbolic code for programmatic handling.
        code: BuildErrorCode,
        /// Invalid rule identity when available from Core or constructor context.
        rule_id: Option<RuleId>,
        /// Original Core diagnostic; not intended for machine parsing.
        detail: String,
    },
}

impl fmt::Display for ScannerBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self::Rejected { detail, .. } = self;
        f.write_str(detail)
    }
}

impl Error for ScannerBuildError {}

impl ScannerConfig {
    pub(crate) fn build_core(self) -> Result<cribra::Scanner, ScannerBuildError> {
        let (include_builtins, rules) = match self {
            Self::BuiltInOnly => (true, Vec::new()),
            Self::CustomOnly { rules } => (false, rules),
            Self::BuiltInAndCustom { rules } => (true, rules),
        };
        let mut builder = cribra::Scanner::builder();
        if include_builtins {
            builder = builder.builtins(cribra::builtins::CURRENT);
        }
        for rule in rules {
            builder = builder.rule(rule.into_core()?);
        }
        builder.build().map_err(ScannerBuildError::from)
    }
}

impl RuleSpec {
    fn into_core(self) -> Result<cribra::Rule, ScannerBuildError> {
        let severity = self.severity.into();
        let rule = match self.kind {
            RuleKind::Literal => cribra::Rule::literal(self.id.value, self.value, severity),
            RuleKind::Prefix => cribra::Rule::prefix(self.id.value, self.value, severity),
            RuleKind::Suffix => cribra::Rule::suffix(self.id.value, self.value, severity),
            RuleKind::Pattern => cribra::Rule::pattern(self.id.value.clone(), self.value, severity)
                .map_err(|error| ScannerBuildError::from_rule(error, Some(self.id)))?,
        };
        Ok(match self.remediation {
            Some(remediation) => rule.with_remediation(remediation.into()),
            None => rule,
        })
    }
}

impl ScannerBuildError {
    fn from_rule(error: cribra::RuleError, rule_id: Option<RuleId>) -> Self {
        let code = match &error {
            cribra::RuleError::InvalidPattern(_) => BuildErrorCode::InvalidPattern,
            cribra::RuleError::PatternMatchesEmpty => BuildErrorCode::PatternMatchesEmpty,
            cribra::RuleError::MissingCaptureGroup { .. } => BuildErrorCode::MissingCaptureGroup,
        };
        Self::Rejected {
            code,
            rule_id,
            detail: error.to_string(),
        }
    }
}

impl From<cribra::ScannerBuildError> for ScannerBuildError {
    fn from(error: cribra::ScannerBuildError) -> Self {
        use cribra::ScannerBuildError as Core;
        let detail = error.to_string();
        let (code, rule_id) = match error {
            Core::Rule(error) => return Self::from_rule(error, None),
            Core::EmptyRuleId => (BuildErrorCode::EmptyRuleId, None),
            Core::DuplicateRuleId { rule_id } => {
                (BuildErrorCode::DuplicateRuleId, Some((&rule_id).into()))
            }
            Core::EmptyMatcher { rule_id } => {
                (BuildErrorCode::EmptyMatcher, Some((&rule_id).into()))
            }
            Core::TooManyRules => (BuildErrorCode::TooManyRules, None),
            Core::AutomatonBuild(_) => (BuildErrorCode::AutomatonBuild, None),
        };
        Self::Rejected {
            code,
            rule_id,
            detail,
        }
    }
}

impl From<Severity> for cribra::Severity {
    fn from(value: Severity) -> Self {
        match value {
            Severity::Info => Self::Info,
            Severity::Low => Self::Low,
            Severity::Medium => Self::Medium,
            Severity::High => Self::High,
            Severity::Critical => Self::Critical,
        }
    }
}

impl From<Remediation> for cribra::Remediation {
    fn from(value: Remediation) -> Self {
        match value {
            Remediation::RevokeAndRotateCredential => Self::RevokeAndRotateCredential,
            Remediation::RotateCredential => Self::RotateCredential,
            Remediation::RotatePassword => Self::RotatePassword,
            Remediation::ReplacePrivateKey => Self::ReplacePrivateKey,
            Remediation::RemoveSensitiveValue => Self::RemoveSensitiveValue,
            Remediation::ReviewSensitiveHash => Self::ReviewSensitiveHash,
            Remediation::ReviewPasswordVerifier => Self::ReviewPasswordVerifier,
        }
    }
}

impl From<cribra::RuleMetadata<'_>> for RuleMetadata {
    fn from(value: cribra::RuleMetadata<'_>) -> Self {
        Self {
            id: RuleId {
                value: value.id().to_owned(),
            },
            kind: match value.kind() {
                cribra::RuleKind::Literal => RuleKind::Literal,
                cribra::RuleKind::Prefix => RuleKind::Prefix,
                cribra::RuleKind::Suffix => RuleKind::Suffix,
                cribra::RuleKind::Pattern => RuleKind::Pattern,
            },
            detection_mode: value.detection_mode().into(),
            severity: value.severity().into(),
            remediation: value.remediation().map(|remediation| RemediationMetadata {
                label: remediation.label().to_owned(),
                message: remediation.message().to_owned(),
            }),
        }
    }
}
