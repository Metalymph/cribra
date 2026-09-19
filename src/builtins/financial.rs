//! Opt-in built-in rules for sensitive financial identifiers.
//!
//! Financial rules are intentionally separate from [`crate::builtins::CURRENT`].
//! Consumers opt into this pack explicitly when financial-data detection is
//! appropriate for their application.

use crate::{Remediation, RuleSpec, Severity, validators::dispatch::ValidatorKind};

/// International Bank Account Number (IBAN).
///
/// Candidate discovery is deliberately broader than structural validation.
/// Country registration, country-specific length and MOD-97 validation remain
/// authoritative in the deterministic IBAN validator.
///
/// Both electronic IBANs and representations containing ASCII spaces are
/// discoverable. The named capture keeps surrounding boundary context out of
/// the resulting finding span.
pub const IBAN: RuleSpec = RuleSpec::captured_pattern(
    "financial.iban",
    r"(?:^|[^A-Za-z0-9])(?P<value>[A-Z]{2}[0-9]{2}[A-Z0-9]{11,29})(?:$|[^A-Za-z0-9])",
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::Iban)
.with_remediation(Remediation::RemoveSensitiveValue);

/// Financial identifier rules available as an explicit opt-in pack.
pub const CURRENT: &[RuleSpec] = &[IBAN];
