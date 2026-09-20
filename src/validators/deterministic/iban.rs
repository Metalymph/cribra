//! Structural validation for International Bank Account Numbers (IBANs).
//!
//! This module recognizes structurally valid IBANs using their registered
//! country code, country-specific length, and ISO 13616 check digits. It does
//! not prove that an account exists, is active, belongs to a particular
//! person, or represents exposed financial information.
//!
//! Country-specific lengths are projected from the SWIFT IBAN Registry
//! snapshot used for Cribra v0.4.6. SWIFT is the ISO 13616 Registration
//! Authority. The table is versioned with Cribra and is never fetched at
//! runtime.

const MIN_IBAN_LEN: usize = 15;
const MAX_IBAN_LEN: usize = 33;

/// Successful structural IBAN validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct IbanValidation;

/// Validates a possible IBAN.
///
/// Both the electronic representation and representations containing ASCII
/// spaces are accepted. Spaces are removed only for validation; the scanner
/// remains responsible for preserving the original authoritative source span.
///
/// No other separator or whitespace character is accepted.
pub(crate) fn validate_iban(candidate: &str) -> Option<IbanValidation> {
    let mut normalized = [0_u8; MAX_IBAN_LEN];
    let normalized_len = normalize(candidate, &mut normalized)?;
    let iban = &normalized[..normalized_len];

    if iban.len() < MIN_IBAN_LEN || iban.len() > MAX_IBAN_LEN {
        return None;
    }

    if !iban[0].is_ascii_uppercase()
        || !iban[1].is_ascii_uppercase()
        || !iban[2].is_ascii_digit()
        || !iban[3].is_ascii_digit()
    {
        return None;
    }

    let expected_len = country_iban_length([iban[0], iban[1]])?;
    if iban.len() != expected_len {
        return None;
    }

    if !iban.iter().all(u8::is_ascii_alphanumeric) {
        return None;
    }

    if !valid_mod97(iban) {
        return None;
    }

    Some(IbanValidation)
}

fn normalize(candidate: &str, output: &mut [u8; MAX_IBAN_LEN]) -> Option<usize> {
    if candidate.is_empty()
        || candidate.as_bytes().first() == Some(&b' ')
        || candidate.as_bytes().last() == Some(&b' ')
    {
        return None;
    }

    let mut len = 0;
    let mut previous_was_space = false;

    for byte in candidate.bytes() {
        if byte == b' ' {
            if previous_was_space {
                return None;
            }

            previous_was_space = true;
            continue;
        }

        if !byte.is_ascii_alphanumeric() || len == MAX_IBAN_LEN {
            return None;
        }

        output[len] = byte;
        len += 1;
        previous_was_space = false;
    }

    Some(len)
}

fn valid_mod97(iban: &[u8]) -> bool {
    let mut remainder = 0_u32;

    for &byte in iban[4..].iter().chain(iban[..4].iter()) {
        if byte.is_ascii_digit() {
            remainder = (remainder * 10 + u32::from(byte - b'0')) % 97;
        } else if byte.is_ascii_uppercase() {
            let value = u32::from(byte - b'A') + 10;
            remainder = (remainder * 10 + value / 10) % 97;
            remainder = (remainder * 10 + value % 10) % 97;
        } else {
            return false;
        }
    }

    remainder == 1
}

fn country_iban_length(country: [u8; 2]) -> Option<usize> {
    const LENGTHS: &[([u8; 2], u8)] = &[
        (*b"AD", 24),
        (*b"AE", 23),
        (*b"AL", 28),
        (*b"AT", 20),
        (*b"AZ", 28),
        (*b"BA", 20),
        (*b"BE", 16),
        (*b"BG", 22),
        (*b"BH", 22),
        (*b"BI", 27),
        (*b"BR", 29),
        (*b"BY", 28),
        (*b"CH", 21),
        (*b"CR", 22),
        (*b"CY", 28),
        (*b"CZ", 24),
        (*b"DE", 22),
        (*b"DJ", 27),
        (*b"DK", 18),
        (*b"DO", 28),
        (*b"EE", 20),
        (*b"EG", 29),
        (*b"ES", 24),
        (*b"FI", 18),
        (*b"FK", 18),
        (*b"FO", 18),
        (*b"FR", 27),
        (*b"GB", 22),
        (*b"GE", 22),
        (*b"GI", 23),
        (*b"GL", 18),
        (*b"GR", 27),
        (*b"GT", 28),
        (*b"HN", 28),
        (*b"HR", 21),
        (*b"HU", 28),
        (*b"IE", 22),
        (*b"IL", 23),
        (*b"IQ", 23),
        (*b"IS", 26),
        (*b"IT", 27),
        (*b"JO", 30),
        (*b"KW", 30),
        (*b"KZ", 20),
        (*b"LB", 28),
        (*b"LC", 32),
        (*b"LI", 21),
        (*b"LT", 20),
        (*b"LU", 20),
        (*b"LV", 21),
        (*b"LY", 25),
        (*b"MC", 27),
        (*b"MD", 24),
        (*b"ME", 22),
        (*b"MK", 19),
        (*b"MN", 20),
        (*b"MR", 27),
        (*b"MT", 31),
        (*b"MU", 30),
        (*b"NI", 28),
        (*b"NL", 18),
        (*b"NO", 15),
        (*b"OM", 23),
        (*b"PK", 24),
        (*b"PL", 28),
        (*b"PS", 29),
        (*b"PT", 25),
        (*b"QA", 29),
        (*b"RO", 24),
        (*b"RS", 22),
        (*b"RU", 33),
        (*b"SA", 24),
        (*b"SC", 31),
        (*b"SD", 18),
        (*b"SE", 24),
        (*b"SI", 19),
        (*b"SK", 24),
        (*b"SM", 27),
        (*b"SO", 23),
        (*b"ST", 25),
        (*b"SV", 28),
        (*b"TL", 23),
        (*b"TN", 24),
        (*b"TR", 26),
        (*b"UA", 29),
        (*b"VA", 22),
        (*b"VG", 24),
        (*b"XK", 20),
        (*b"YE", 30),
    ];

    LENGTHS
        .iter()
        .find_map(|&(code, len)| (code == country).then_some(usize::from(len)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_registry_electronic_examples() {
        let cases = [
            "NO9386011117947",
            "BE68539007547034",
            "DE89370400440532013000",
            "GB29NWBK60161331926819",
            "IT60X0542811101000000123456",
            "MT84MALT011000012345MTLCAST001S",
            "RU0304452522540817810538091310419",
            "XK051212012345678906",
            "YE15CBYE0001018861234567891234",
        ];

        for candidate in cases {
            assert!(
                validate_iban(candidate).is_some(),
                "registry example should validate"
            );
        }
    }

    #[test]
    fn accepts_registry_print_examples() {
        let cases = [
            "AD12 0001 2030 2003 5910 0100",
            "IT60 X054 2811 1010 0000 0123 456",
            "LY83 002 048 000020100120361",
            "MT84 MALT 0110 0001 2345 MTLC AST0 01S",
            "RU03 0445 2522 5408 1781 0538 0913 1041 9",
        ];

        for candidate in cases {
            assert!(
                validate_iban(candidate).is_some(),
                "registry print example should validate"
            );
        }
    }

    #[test]
    fn rejects_unknown_country() {
        assert!(validate_iban("ZZ89370400440532013000").is_none());
    }

    #[test]
    fn rejects_wrong_country_length() {
        assert!(validate_iban("DE8937040044053201300").is_none());
        assert!(validate_iban("DE893704004405320130000").is_none());
    }

    #[test]
    fn rejects_invalid_check_digits() {
        assert!(validate_iban("DE00370400440532013000").is_none());
        assert!(validate_iban("IT00X0542811101000000123456").is_none());
    }

    #[test]
    fn rejects_invalid_alphabet() {
        assert!(validate_iban("DE8937040044053201300-").is_none());
        assert!(validate_iban("IT60X054281110100000012345_").is_none());
    }

    #[test]
    fn rejects_lowercase() {
        assert!(validate_iban("de89370400440532013000").is_none());
        assert!(validate_iban("IT60x0542811101000000123456").is_none());
    }

    #[test]
    fn rejects_unsupported_whitespace_and_spacing() {
        assert!(validate_iban(" DE89370400440532013000").is_none());
        assert!(validate_iban("DE89370400440532013000 ").is_none());
        assert!(validate_iban("DE89  3704 0044 0532 0130 00").is_none());
        assert!(validate_iban("DE89\t3704\t0044\t0532\t0130\t00").is_none());
        assert!(validate_iban("DE89\n3704 0044 0532 0130 00").is_none());
    }

    #[test]
    fn rejects_checksum_preserving_country_mismatch_through_length_authority() {
        // A structurally valid German example cannot simply be relabeled with a
        // country whose registered IBAN length differs.
        assert!(validate_iban("NO89370400440532013000").is_none());
    }

    #[test]
    fn country_length_table_covers_registry_snapshot() {
        assert_eq!(country_iban_length(*b"AD"), Some(24));
        assert_eq!(country_iban_length(*b"NO"), Some(15));
        assert_eq!(country_iban_length(*b"RU"), Some(33));
        assert_eq!(country_iban_length(*b"XK"), Some(20));
        assert_eq!(country_iban_length(*b"YE"), Some(30));
        assert_eq!(country_iban_length(*b"ZZ"), None);
    }
}
