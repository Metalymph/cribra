//! Contextual validation for payment-card verification codes.
//!
//! A bare three- or four-digit value is not sufficient evidence of payment-card
//! verification material. Validation therefore requires an explicit
//! card-verification field name.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};

const CARD_VERIFICATION_CODE_KEYS: &[&str] = &[
    "cvv",
    "cvv2",
    "cvc",
    "cvc2",
    "cid",
    "card_verification_code",
    "card_verification_value",
    "card_security_code",
    "card_security_value",
];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct CardVerificationCodeValidation;

/// Validates a three- or four-digit payment-card verification code in explicit
/// card-verification context.
pub(crate) fn validate_card_verification_code(
    context: &ValidationContext<'_>,
) -> Option<CardVerificationCodeValidation> {
    let candidate = context.candidate();

    if !matches!(candidate.len(), 3 | 4) || !candidate.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    if !key_matches_any(key, CARD_VERIFICATION_CODE_KEYS) {
        return None;
    }

    Some(CardVerificationCodeValidation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, candidate: &'a str) -> ValidationContext<'a> {
        let start = source
            .find(candidate)
            .expect("fixture must contain candidate");

        ValidationContext::new(source, start..start + candidate.len())
    }

    #[test]
    fn accepts_three_and_four_digit_codes_in_explicit_context() {
        for (key, value) in [
            ("cvv", "123"),
            ("cvv2", "123"),
            ("cvc", "123"),
            ("cvc2", "123"),
            ("cid", "1234"),
            ("card-verification-code", "123"),
            ("card.verification.value", "123"),
            ("card_security_code", "123"),
            ("card-security-value", "1234"),
        ] {
            let source = format!("{key}={value}");

            assert!(
                validate_card_verification_code(&context(&source, value)).is_some(),
                "expected card-verification context for key {key:?}",
            );
        }
    }

    #[test]
    fn rejects_weak_or_unrelated_context() {
        for key in [
            "code",
            "security_code",
            "verification_code",
            "pin",
            "card",
            "card_code",
        ] {
            let source = format!("{key}=123");

            assert!(
                validate_card_verification_code(&context(&source, "123")).is_none(),
                "unexpected card-verification context for key {key:?}",
            );
        }
    }

    #[test]
    fn rejects_non_numeric_and_wrong_width_values() {
        for value in ["12", "12345", "12x", "1 23", "1-23"] {
            let source = format!("cvv={value}");

            assert!(
                validate_card_verification_code(&context(&source, value)).is_none(),
                "unexpected acceptance for {value:?}",
            );
        }
    }
}
