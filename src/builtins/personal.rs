//! Opt-in built-in rules for structured personal identifiers.
//!
//! Personal-data rules are intentionally separate from
//! [`crate::builtins::CURRENT`]. Consumers opt into this pack explicitly when
//! structured personal-data detection is appropriate for their application.

use crate::{Remediation, RuleSpec, Severity, validators::dispatch::ValidatorKind};

/// Italian 16-character Codice Fiscale for natural persons.
///
/// Discovery intentionally accepts a broader 16-character uppercase
/// alphanumeric candidate. Positional structure, omocodia, encoded birth data,
/// birthplace-code structure, and the control character remain authoritative
/// in the deterministic validator.
///
/// This rule does not cover 11-digit numeric tax identifiers.
pub const CODICE_FISCALE: RuleSpec = RuleSpec::captured_pattern(
    "personal.it-codice-fiscale",
    r"(?:^|[^A-Za-z0-9])(?P<value>[A-Z0-9]{16})(?:$|[^A-Za-z0-9])",
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::CodiceFiscale)
.with_remediation(Remediation::RemoveSensitiveValue);

/// Polish 11-digit PESEL identifier.
///
/// Discovery accepts an isolated 11-digit candidate. Century/month decoding,
/// Gregorian birth-date validity, and the checksum remain authoritative in the
/// deterministic validator.
///
/// Structural validity does not establish assignment or registry presence.
pub const PESEL: RuleSpec = RuleSpec::captured_pattern(
    "personal.pl-pesel",
    r"(?:^|[^A-Za-z0-9])(?P<value>[0-9]{11})(?:$|[^A-Za-z0-9])",
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::Pesel)
.with_remediation(Remediation::RemoveSensitiveValue);

/// UK NHS Number under explicit NHS-number context.
///
/// Discovery supports compact 10-digit values and canonical 3-3-4
/// representations separated by ASCII spaces or hyphens. Validation requires
/// an NHS-specific field context and a valid Modulus 11 check digit.
///
/// Structural validity does not establish assignment, patient identity, or
/// presence in an authoritative NHS registry.
pub const NHS_NUMBER: RuleSpec = RuleSpec::captured_pattern(
    "personal.uk-nhs-number",
    r"(?:^|[^A-Za-z0-9])(?P<value>[0-9]{10}|[0-9]{3} [0-9]{3} [0-9]{4}|[0-9]{3}-[0-9]{3}-[0-9]{4})(?:$|[^A-Za-z0-9])",
    "value",
    Severity::High,
)
.with_validator(ValidatorKind::NhsNumber)
.with_remediation(Remediation::RemoveSensitiveValue);

/// Structured personal-identifier rules available as an explicit opt-in pack.
pub const CURRENT: &[RuleSpec] = &[CODICE_FISCALE, PESEL, NHS_NUMBER];
