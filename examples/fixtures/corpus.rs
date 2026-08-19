//! Shared definition of the canonical Cribra fixture corpus.
//!
//! This module is intentionally outside the public crate API. It is compiled by
//! both `examples/generate_fixtures.rs` and `tests/golden.rs`, ensuring that the
//! generator and verifier use the exact same scanner configuration and
//! deterministic transformation keys.

use cribra::{Remediation, Rule, Scanner, Severity};

pub const PSEUDONYMIZATION_KEY: [u8; 32] = [0x31; 32];
pub const SYNTHESIS_KEY: [u8; 32] = [0x53; 32];

pub fn scanner() -> Result<Scanner, cribra::ScannerBuildError> {
    Scanner::builder()
        .rule(
            Rule::prefix("demo.api-key", "demo_api_", Severity::Critical)
                .with_remediation(Remediation::RotateCredential),
        )
        .rule(
            Rule::pattern(
                "demo.password",
                r"demo-pass-[A-Za-z0-9_\-\p{L}]+",
                Severity::High,
            )
            .expect("canonical password pattern must compile")
            .with_remediation(Remediation::RotatePassword),
        )
        .rule(
            Rule::literal(
                "demo.private-key",
                "DEMO_PRIVATE_KEY_MATERIAL",
                Severity::Critical,
            )
            .with_remediation(Remediation::ReplacePrivateKey),
        )
        .rule(
            Rule::literal("demo.secret", "DEMO_SECRET_ALPHA", Severity::High)
                .with_remediation(Remediation::RemoveSensitiveValue),
        )
        .build()
}
