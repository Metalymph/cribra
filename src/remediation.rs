//! Structured remediation guidance attached to findings.
//!
//! Remediation describes the security action recommended after a detection.
//! It is intentionally separate from transformations such as redaction:
//! remediation addresses the underlying exposure, while transformations create
//! safer representations of source material.

use core::fmt;

/// Recommended response to a detected sensitive value.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[non_exhaustive]
pub enum Remediation {
    /// Revoke the exposed credential and issue a replacement.
    RevokeAndRotateCredential,
    /// Rotate the credential where revocation is not a distinct provider action.
    RotateCredential,
    /// Replace the exposed password or passphrase.
    RotatePassword,
    /// Replace the exposed private key and retire the previous key.
    ReplacePrivateKey,
    /// Remove the sensitive value from material that should not contain it.
    RemoveSensitiveValue,
    /// Review whether the detected hash is appropriate to expose or distribute.
    ReviewSensitiveHash,
}

impl Remediation {
    /// Returns a short, stable, human-readable action label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RevokeAndRotateCredential => "Revoke and rotate credential",
            Self::RotateCredential => "Rotate credential",
            Self::RotatePassword => "Rotate password",
            Self::ReplacePrivateKey => "Replace private key",
            Self::RemoveSensitiveValue => "Remove sensitive value",
            Self::ReviewSensitiveHash => "Review sensitive hash",
        }
    }

    /// Returns concise remediation guidance suitable for direct presentation.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::RevokeAndRotateCredential => {
                "Revoke and rotate this credential. If it was committed or shared, treat it as exposed."
            }
            Self::RotateCredential => {
                "Rotate this credential and replace every active use of the previous value."
            }
            Self::RotatePassword => {
                "Replace this password or passphrase and update every system that still uses it."
            }
            Self::ReplacePrivateKey => {
                "Replace this private key and retire the previous key wherever it is trusted."
            }
            Self::RemoveSensitiveValue => {
                "Remove this sensitive value before storing, publishing, or sharing this content."
            }
            Self::ReviewSensitiveHash => {
                "Review whether this hash represents sensitive information before sharing it."
            }
        }
    }
}

impl fmt::Display for Remediation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_remediation_has_presentation_text() {
        let variants = [
            Remediation::RevokeAndRotateCredential,
            Remediation::RotateCredential,
            Remediation::RotatePassword,
            Remediation::ReplacePrivateKey,
            Remediation::RemoveSensitiveValue,
            Remediation::ReviewSensitiveHash,
        ];

        for remediation in variants {
            assert!(!remediation.label().is_empty());
            assert!(!remediation.message().is_empty());
            assert_eq!(remediation.to_string(), remediation.message());
        }
    }
}
