//! Contextual validation for US Social Security Numbers.
//!
//! A structurally possible nine-digit value is not sufficient evidence of an
//! SSN on its own. Validation therefore requires both current SSA structural
//! constraints and explicit Social Security Number field context.
//!
//! Structural validity does not establish assignment, holder identity, or
//! presence in authoritative SSA records.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};

const SSN_LEN: usize = 9;

const SSN_KEYS: &[&str] = &["ssn", "social_security_number", "socialsecuritynumber"];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct SsnValidation;

/// Validates a US SSN in explicit Social Security Number field context.
pub(crate) fn validate_ssn(context: &ValidationContext<'_>) -> Option<SsnValidation> {
    let digits = normalize_ssn(context.candidate())?;

    if !valid_structure(&digits) {
        return None;
    }

    let key = nearest_key(context.before_window(DEFAULT_KEY_WINDOW))?;

    if !key_matches_any(key, SSN_KEYS) {
        return None;
    }

    Some(SsnValidation)
}

fn normalize_ssn(candidate: &str) -> Option<[u8; SSN_LEN]> {
    let bytes = candidate.as_bytes();
    let mut digits = [0_u8; SSN_LEN];

    match bytes {
        // Compact representation: AAAGGSSSS.
        [a, b, c, d, e, f, g, h, i] if bytes.iter().all(u8::is_ascii_digit) => {
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
            ]);
        }

        // Canonical representation: AAA-GG-SSSS.
        [a, b, c, b'-', d, e, b'-', f, g, h, i]
            if [a, b, c, d, e, f, g, h, i]
                .iter()
                .all(|byte| byte.is_ascii_digit()) =>
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
            ]);
        }

        _ => return None,
    }

    Some(digits)
}

fn valid_structure(digits: &[u8; SSN_LEN]) -> bool {
    let area = u16::from(digits[0]) * 100 + u16::from(digits[1]) * 10 + u16::from(digits[2]);
    let group = digits[3] * 10 + digits[4];
    let serial = u16::from(digits[5]) * 1000
        + u16::from(digits[6]) * 100
        + u16::from(digits[7]) * 10
        + u16::from(digits[8]);

    area != 0 && area != 666 && area < 900 && group != 0 && serial != 0
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
    fn accepts_structurally_valid_ssn_in_explicit_context() {
        for ssn in ["123456789", "123-45-6789"] {
            for key in [
                "ssn",
                "social_security_number",
                "social-security-number",
                "social.security.number",
                "socialsecuritynumber",
            ] {
                let source = format!("{key}={ssn}");

                assert!(
                    validate_ssn(&context(&source, ssn)).is_some(),
                    "expected SSN context for key {key:?} and value {ssn:?}",
                );
            }
        }
    }

    #[test]
    fn rejects_bare_structurally_valid_ssn() {
        for ssn in ["123456789", "123-45-6789"] {
            assert!(
                validate_ssn(&context(ssn, ssn)).is_none(),
                "bare SSN-like value {ssn:?} must not be classified",
            );
        }
    }

    #[test]
    fn rejects_unrelated_or_weak_context() {
        let ssn = "123-45-6789";

        for key in [
            "number",
            "id",
            "tax_id",
            "tax_number",
            "national_id",
            "social_security",
            "employee_id",
            "person_id",
        ] {
            let source = format!("{key}={ssn}");

            assert!(
                validate_ssn(&context(&source, ssn)).is_none(),
                "unexpected SSN context for key {key:?}",
            );
        }
    }

    #[test]
    fn rejects_invalid_area_numbers() {
        for ssn in [
            "000-45-6789",
            "666-45-6789",
            "900-45-6789",
            "950-45-6789",
            "999-45-6789",
        ] {
            let source = format!("ssn={ssn}");

            assert!(
                validate_ssn(&context(&source, ssn)).is_none(),
                "invalid SSN area must be rejected: {ssn:?}",
            );
        }
    }

    #[test]
    fn accepts_area_boundaries_that_remain_structurally_possible() {
        for ssn in ["001-45-6789", "665-45-6789", "667-45-6789", "899-45-6789"] {
            let source = format!("ssn={ssn}");

            assert!(
                validate_ssn(&context(&source, ssn)).is_some(),
                "structurally possible SSN area must remain accepted: {ssn:?}",
            );
        }
    }

    #[test]
    fn rejects_zero_group() {
        for ssn in ["123-00-6789", "123006789"] {
            let source = format!("ssn={ssn}");

            assert!(
                validate_ssn(&context(&source, ssn)).is_none(),
                "zero SSN group must be rejected: {ssn:?}",
            );
        }
    }

    #[test]
    fn rejects_zero_serial() {
        for ssn in ["123-45-0000", "123450000"] {
            let source = format!("ssn={ssn}");

            assert!(
                validate_ssn(&context(&source, ssn)).is_none(),
                "zero SSN serial must be rejected: {ssn:?}",
            );
        }
    }

    #[test]
    fn rejects_noncanonical_formatting() {
        for ssn in [
            "123 45 6789",
            "123.45.6789",
            "123-456-789",
            "1234-56-789",
            "123-45 6789",
        ] {
            let source = format!("ssn={ssn}");

            assert!(
                validate_ssn(&context(&source, ssn)).is_none(),
                "noncanonical SSN representation {ssn:?} must be rejected",
            );
        }
    }

    #[test]
    fn rejects_wrong_digit_count_and_non_digits() {
        for ssn in ["12345678", "1234567890", "12345678A", "123-45-678A"] {
            let source = format!("ssn={ssn}");

            assert!(
                validate_ssn(&context(&source, ssn)).is_none(),
                "invalid SSN representation {ssn:?} must be rejected",
            );
        }
    }
}
