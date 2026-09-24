//! Structural validation for the Italian Codice Fiscale.
//!
//! This validator recognizes the canonical 16-character representation used
//! for natural persons. It validates positional structure, supported omocodia
//! substitutions, encoded month/day fields, birthplace-code structure, and
//! the final control character.
//!
//! Structural validity does not prove that the identifier was assigned, that
//! the represented person exists, or that the identifier is currently valid
//! in an authoritative registry.

const CF_LEN: usize = 16;

/// Successful structural Codice Fiscale validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct CodiceFiscaleValidation;

/// Validates a possible 16-character Italian Codice Fiscale.
pub(crate) fn validate_codice_fiscale(candidate: &str) -> Option<CodiceFiscaleValidation> {
    let bytes = candidate.as_bytes();

    if bytes.len() != CF_LEN
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return None;
    }

    // Surname and name encodings.
    if !bytes[..6].iter().all(u8::is_ascii_uppercase) {
        return None;
    }

    // Year: positions 6–7 are numeric positions and may use omocodia.
    let year_tens = decode_numeric_position(bytes[6])?;
    let year_units = decode_numeric_position(bytes[7])?;

    // Keep the decoded year explicit even though every 00–99 value is
    // structurally admissible.
    let _year = year_tens * 10 + year_units;

    // Month letter.
    if !matches!(
        bytes[8],
        b'A' | b'B' | b'C' | b'D' | b'E' | b'H' | b'L' | b'M' | b'P' | b'R' | b'S' | b'T'
    ) {
        return None;
    }

    // Day/sex encoding: 01–31 for male, 41–71 for female.
    let day_tens = decode_numeric_position(bytes[9])?;
    let day_units = decode_numeric_position(bytes[10])?;
    let encoded_day = day_tens * 10 + day_units;

    if !matches!(encoded_day, 1..=31 | 41..=71) {
        return None;
    }

    // Birthplace/cadastral code: one letter followed by three numeric
    // positions. The numeric positions may use omocodia.
    if !bytes[11].is_ascii_uppercase() {
        return None;
    }

    decode_numeric_position(bytes[12])?;
    decode_numeric_position(bytes[13])?;
    decode_numeric_position(bytes[14])?;

    let expected = control_character(&bytes[..15])?;

    if bytes[15] != expected {
        return None;
    }

    Some(CodiceFiscaleValidation)
}

fn decode_numeric_position(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'L' => Some(0),
        b'M' => Some(1),
        b'N' => Some(2),
        b'P' => Some(3),
        b'Q' => Some(4),
        b'R' => Some(5),
        b'S' => Some(6),
        b'T' => Some(7),
        b'U' => Some(8),
        b'V' => Some(9),
        _ => None,
    }
}

fn control_character(bytes: &[u8]) -> Option<u8> {
    if bytes.len() != 15 {
        return None;
    }

    let mut sum = 0_u32;

    for (index, &byte) in bytes.iter().enumerate() {
        // Codice Fiscale positions are conventionally numbered from 1.
        // Rust indices 0, 2, 4, ... therefore correspond to odd positions.
        let value = if index % 2 == 0 {
            odd_position_value(byte)?
        } else {
            even_position_value(byte)?
        };

        sum += u32::from(value);
    }

    Some(b'A' + (sum % 26) as u8)
}

fn even_position_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'Z' => Some(byte - b'A'),
        _ => None,
    }
}

fn odd_position_value(byte: u8) -> Option<u8> {
    Some(match byte {
        b'0' | b'A' => 1,
        b'1' | b'B' => 0,
        b'2' | b'C' => 5,
        b'3' | b'D' => 7,
        b'4' | b'E' => 9,
        b'5' | b'F' => 13,
        b'6' | b'G' => 15,
        b'7' | b'H' => 17,
        b'8' | b'I' => 19,
        b'9' | b'J' => 21,
        b'K' => 2,
        b'L' => 4,
        b'M' => 18,
        b'N' => 20,
        b'O' => 11,
        b'P' => 3,
        b'Q' => 6,
        b'R' => 8,
        b'S' => 12,
        b'T' => 14,
        b'U' => 16,
        b'V' => 10,
        b'W' => 22,
        b'X' => 25,
        b'Y' => 24,
        b'Z' => 23,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_structurally_valid_codice_fiscale() {
        assert!(validate_codice_fiscale("RSSMRA85T10A562S").is_some());
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(validate_codice_fiscale("RSSMRA85T10A562").is_none());
        assert!(validate_codice_fiscale("RSSMRA85T10A562SS").is_none());
    }

    #[test]
    fn rejects_lowercase() {
        assert!(validate_codice_fiscale("rssmra85t10a562s").is_none());
    }

    #[test]
    fn rejects_invalid_name_encoding() {
        assert!(validate_codice_fiscale("RSSMR185T10A562S").is_none());
    }

    #[test]
    fn rejects_invalid_month_code() {
        assert!(validate_codice_fiscale("RSSMRA85Z10A562S").is_none());
    }

    #[test]
    fn rejects_invalid_encoded_day() {
        assert!(validate_codice_fiscale("RSSMRA85T00A562S").is_none());
        assert!(validate_codice_fiscale("RSSMRA85T32A562S").is_none());
        assert!(validate_codice_fiscale("RSSMRA85T40A562S").is_none());
        assert!(validate_codice_fiscale("RSSMRA85T72A562S").is_none());
    }

    #[test]
    fn rejects_invalid_birthplace_structure() {
        assert!(validate_codice_fiscale("RSSMRA85T101562S").is_none());
        assert!(validate_codice_fiscale("RSSMRA85T10AA62S").is_none());
    }

    #[test]
    fn accepts_valid_omocodic_codice_fiscale() {
        assert!(validate_codice_fiscale("RSSMRA85T10A56NH").is_some());
    }

    #[test]
    fn rejects_invalid_omocodia_letter_in_numeric_position() {
        assert!(validate_codice_fiscale("RSSMRAW5T10A562S").is_none());
    }

    #[test]
    fn rejects_wrong_control_character() {
        assert!(validate_codice_fiscale("RSSMRA85T10A562A").is_none());
    }

    #[test]
    fn rejects_numeric_eleven_character_tax_identifier() {
        assert!(validate_codice_fiscale("12345678901").is_none());
    }
}
