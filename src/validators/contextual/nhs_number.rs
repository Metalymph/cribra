//! Contextual validation for UK NHS Numbers.
//!
//! A checksum-valid 10-digit sequence is not sufficient evidence of an NHS
//! Number on its own. Validation therefore requires both NHS Number structure
//! and explicit NHS-number context.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};

const NHS_NUMBER_LEN: usize = 10;

const NHS_NUMBER_KEYS: &[&str] = &["nhs_number", "nhs_no", "nhs_num", "nhsnumber"];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct NhsNumberValidation;

/// Validates an NHS Number in explicit NHS-number context.
pub(crate) fn validate_nhs_number(context: &ValidationContext<'_>) -> Option<NhsNumberValidation> {
    let digits = normalize_nhs_number(context.candidate())?;

    if is_known_placeholder(&digits) || !valid_checksum(&digits) {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    if !key_matches_any(key, NHS_NUMBER_KEYS) {
        return None;
    }

    Some(NhsNumberValidation)
}

fn normalize_nhs_number(candidate: &str) -> Option<[u8; NHS_NUMBER_LEN]> {
    let bytes = candidate.as_bytes();
    let mut digits = [0_u8; NHS_NUMBER_LEN];

    match bytes {
        // Compact representation.
        [a, b, c, d, e, f, g, h, i, j] if bytes.iter().all(u8::is_ascii_digit) => {
            digits.copy_from_slice(&[
                *a - b'0',
                *b - b'0',
                *c - b'0',
                *d - b'0',
                *e - b'0',
                *f - b'0',
                *g - b'0',
                *h - b'0',
                *i - b'0',
                *j - b'0',
            ]);
        }

        // Canonical 3-3-4 representation using either spaces or hyphens.
        [a, b, c, sep1, d, e, f, sep2, g, h, i, j]
            if [a, b, c, d, e, f, g, h, i, j]
                .iter()
                .all(|byte| byte.is_ascii_digit())
                && (*sep1 == b' ' || *sep1 == b'-')
                && sep1 == sep2 =>
        {
            digits.copy_from_slice(&[
                *a - b'0',
                *b - b'0',
                *c - b'0',
                *d - b'0',
                *e - b'0',
                *f - b'0',
                *g - b'0',
                *h - b'0',
                *i - b'0',
                *j - b'0',
            ]);
        }

        _ => return None,
    }

    Some(digits)
}

fn is_known_placeholder(digits: &[u8; NHS_NUMBER_LEN]) -> bool {
    matches!(
        digits,
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 0] | [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    )
}

fn valid_checksum(digits: &[u8; NHS_NUMBER_LEN]) -> bool {
    const WEIGHTS: [u32; 9] = [10, 9, 8, 7, 6, 5, 4, 3, 2];

    let sum: u32 = digits[..9]
        .iter()
        .zip(WEIGHTS)
        .map(|(&digit, weight)| u32::from(digit) * weight)
        .sum();

    let remainder = sum % 11;
    let check = 11 - remainder;

    let expected = match check {
        11 => 0,
        10 => return false,
        value => value as u8,
    };

    digits[9] == expected
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
    fn accepts_checksum_valid_compact_nhs_number_in_explicit_context() {
        let nhs_number = "9434765919";

        for key in [
            "nhs_number",
            "nhs-number",
            "nhs.no",
            "nhs_no",
            "nhs_num",
            "nhsnumber",
        ] {
            let source = format!("{key}={nhs_number}");

            assert!(
                validate_nhs_number(&context(&source, nhs_number)).is_some(),
                "expected NHS Number context for key {key:?}",
            );
        }
    }

    #[test]
    fn accepts_canonical_spaced_and_hyphenated_representations() {
        for nhs_number in ["943 476 5919", "943-476-5919"] {
            let source = format!("nhs_number={nhs_number}");

            assert!(
                validate_nhs_number(&context(&source, nhs_number)).is_some(),
                "expected canonical NHS Number representation {nhs_number:?}",
            );
        }
    }

    #[test]
    fn rejects_bare_checksum_valid_nhs_number() {
        let nhs_number = "9434765919";

        assert!(validate_nhs_number(&context(nhs_number, nhs_number)).is_none());
    }

    #[test]
    fn rejects_unrelated_or_weak_context() {
        let nhs_number = "9434765919";

        for key in [
            "number",
            "patient",
            "patient_number",
            "account",
            "account_number",
            "reference",
            "reference_number",
        ] {
            let source = format!("{key}={nhs_number}");

            assert!(
                validate_nhs_number(&context(&source, nhs_number)).is_none(),
                "unexpected NHS Number context for key {key:?}",
            );
        }
    }

    #[test]
    fn rejects_invalid_checksum() {
        let nhs_number = "9434765918";
        let source = format!("nhs_number={nhs_number}");

        assert!(validate_nhs_number(&context(&source, nhs_number)).is_none());
    }

    #[test]
    fn rejects_known_placeholders() {
        for nhs_number in ["1234567890", "0123456789"] {
            let source = format!("nhs_number={nhs_number}");

            assert!(
                validate_nhs_number(&context(&source, nhs_number)).is_none(),
                "known placeholder {nhs_number:?} must be rejected",
            );
        }
    }

    #[test]
    fn rejects_noncanonical_formatting() {
        for nhs_number in [
            "943 476-5919",
            "943-476 5919",
            "943.476.5919",
            "943  476 5919",
            "943476 5919",
        ] {
            let source = format!("nhs_number={nhs_number}");

            assert!(
                validate_nhs_number(&context(&source, nhs_number)).is_none(),
                "noncanonical NHS Number representation {nhs_number:?} must be rejected",
            );
        }
    }

    #[test]
    fn rejects_wrong_digit_count_and_non_digits() {
        for nhs_number in ["943476591", "94347659190", "943476591A"] {
            let source = format!("nhs_number={nhs_number}");

            assert!(
                validate_nhs_number(&context(&source, nhs_number)).is_none(),
                "invalid NHS Number representation {nhs_number:?} must be rejected",
            );
        }
    }
}
