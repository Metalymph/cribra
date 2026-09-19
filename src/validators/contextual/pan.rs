//! Contextual validation for payment-card primary account numbers.
//!
//! A Luhn-valid numeric sequence is not sufficient evidence of a PAN on its
//! own. Validation therefore requires both PAN structure and an explicit
//! payment-card field name.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};

const MIN_PAN_LEN: usize = 10;
const MAX_PAN_LEN: usize = 19;

const PAN_KEYS: &[&str] = &[
    "pan",
    "primary_account_number",
    "card_number",
    "card_no",
    "card_num",
    "payment_card_number",
    "credit_card_number",
    "debit_card_number",
];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct PanValidation;

/// Validates a compact payment-card PAN in explicit card-number context.
pub(crate) fn validate_pan(context: &ValidationContext<'_>) -> Option<PanValidation> {
    let candidate = context.candidate();

    if !valid_pan_structure(candidate) || !valid_luhn(candidate.as_bytes()) {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    if !key_matches_any(key, PAN_KEYS) {
        return None;
    }

    Some(PanValidation)
}

fn valid_pan_structure(candidate: &str) -> bool {
    (MIN_PAN_LEN..=MAX_PAN_LEN).contains(&candidate.len())
        && candidate.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_luhn(digits: &[u8]) -> bool {
    let mut sum = 0u32;
    let parity = digits.len() % 2;

    for (index, byte) in digits.iter().copied().enumerate() {
        let mut digit = u32::from(byte - b'0');

        if index % 2 == parity {
            digit *= 2;

            if digit > 9 {
                digit -= 9;
            }
        }

        sum += digit;
    }

    sum.is_multiple_of(10)
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
    fn accepts_luhn_valid_pan_in_explicit_card_context() {
        let pan = "4111111111111111";

        for key in [
            "pan",
            "primary_account_number",
            "card_number",
            "card-no",
            "payment.card.number",
            "credit_card_number",
            "debit-card-number",
        ] {
            let source = format!("{key}={pan}");
            assert!(
                validate_pan(&context(&source, pan)).is_some(),
                "expected PAN context for key {key:?}",
            );
        }
    }

    #[test]
    fn rejects_invalid_luhn_value() {
        let pan = "4111111111111112";
        let source = format!("card_number={pan}");

        assert!(validate_pan(&context(&source, pan)).is_none());
    }

    #[test]
    fn rejects_unrelated_or_weak_context() {
        let pan = "4111111111111111";

        for key in ["number", "account", "account_number", "payment", "card"] {
            let source = format!("{key}={pan}");
            assert!(
                validate_pan(&context(&source, pan)).is_none(),
                "unexpected PAN context for key {key:?}",
            );
        }
    }

    #[test]
    fn rejects_non_numeric_and_out_of_range_values() {
        for pan in [
            "411111111",
            "41111111111111111111",
            "411111111111111x",
            "4111-1111-1111-1111",
            "4111 1111 1111 1111",
        ] {
            let source = format!("card_number={pan}");
            assert!(
                validate_pan(&context(&source, pan)).is_none(),
                "unexpected acceptance for {pan:?}",
            );
        }
    }
}
