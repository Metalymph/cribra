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

/// Payment-card primary account number in compact electronic representation.
///
/// Discovery is intentionally limited to 10–19 consecutive ASCII digits.
/// Validation additionally requires a valid Luhn checksum and explicit
/// payment-card field context. Card-network attribution is outside the rule
/// contract.
pub const PAN: RuleSpec = RuleSpec::captured_pattern(
    "financial.pan",
    r"(?:^|[^0-9])(?P<value>[0-9]{10,19})(?:$|[^0-9])",
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::Pan)
.with_remediation(Remediation::RemoveSensitiveValue);

/// Payment-card verification code.
///
/// Discovery is deliberately limited to three or four consecutive ASCII
/// digits. Validation additionally requires explicit payment-card verification
/// context because a bare short numeric value has no authoritative financial
/// semantics.
pub const CARD_VERIFICATION_CODE: RuleSpec = RuleSpec::captured_pattern(
    "financial.card-verification-code",
    r"(?:^|[^0-9])(?P<value>[0-9]{3,4})(?:$|[^0-9])",
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::CardVerificationCode)
.with_remediation(Remediation::RemoveSensitiveValue);

/// Financial identifier rules available as an explicit opt-in pack.
pub const CURRENT: &[RuleSpec] = &[IBAN, PAN, CARD_VERIFICATION_CODE];
