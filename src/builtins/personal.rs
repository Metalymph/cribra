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

/// Structured personal-identifier rules available as an explicit opt-in pack.
pub const CURRENT: &[RuleSpec] = &[CODICE_FISCALE];
