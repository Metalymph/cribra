//! Structural validation for the Polish PESEL identifier.
//!
//! This validator recognizes the canonical 11-digit representation used for
//! natural persons. It validates the encoded century and birth date together
//! with the final checksum digit.
//!
//! Structural validity does not prove that the identifier was assigned, that
//! the represented person exists, or that the identifier is currently present
//! in the PESEL register.

const PESEL_LEN: usize = 11;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct PeselValidation;

/// Validates a possible 11-digit Polish PESEL identifier.
pub(crate) fn validate_pesel(candidate: &str) -> Option<PeselValidation> {
    let bytes = candidate.as_bytes();

    if bytes.len() != PESEL_LEN || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }

    let mut digits = [0_u8; PESEL_LEN];

    for (index, byte) in bytes.iter().copied().enumerate() {
        digits[index] = byte - b'0';
    }

    let year_suffix = u16::from(digits[0]) * 10 + u16::from(digits[1]);
    let encoded_month = digits[2] * 10 + digits[3];
    let day = digits[4] * 10 + digits[5];

    let (century, month) = decode_century_and_month(encoded_month)?;
    let year = century + year_suffix;

    if !valid_date(year, month, day) {
        return None;
    }

    if checksum(&digits[..10])? != digits[10] {
        return None;
    }

    Some(PeselValidation)
}

fn decode_century_and_month(encoded_month: u8) -> Option<(u16, u8)> {
    match encoded_month {
        1..=12 => Some((1900, encoded_month)),
        21..=32 => Some((2000, encoded_month - 20)),
        41..=52 => Some((2100, encoded_month - 40)),
        61..=72 => Some((2200, encoded_month - 60)),
        81..=92 => Some((1800, encoded_month - 80)),
        _ => None,
    }
}

fn valid_date(year: u16, month: u8, day: u8) -> bool {
    if day == 0 {
        return false;
    }

    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return false,
    };

    day <= max_day
}

fn is_leap_year(year: u16) -> bool {
    year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100))
}

fn checksum(digits: &[u8]) -> Option<u8> {
    const WEIGHTS: [u8; 10] = [1, 3, 7, 9, 1, 3, 7, 9, 1, 3];

    if digits.len() != WEIGHTS.len() {
        return None;
    }

    let sum: u32 = digits
        .iter()
        .zip(WEIGHTS)
        .map(|(&digit, weight)| u32::from(digit) * u32::from(weight))
        .sum();

    Some(((10 - (sum % 10)) % 10) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pesel_for_date(year: u16, month: u8, day: u8, serial: [u8; 4]) -> String {
        let encoded_month = match year {
            1800..=1899 => month + 80,
            1900..=1999 => month,
            2000..=2099 => month + 20,
            2100..=2199 => month + 40,
            2200..=2299 => month + 60,
            _ => panic!("test helper only supports PESEL centuries"),
        };

        let year_suffix = (year % 100) as u8;

        let mut digits = [
            year_suffix / 10,
            year_suffix % 10,
            encoded_month / 10,
            encoded_month % 10,
            day / 10,
            day % 10,
            serial[0],
            serial[1],
            serial[2],
            serial[3],
            0,
        ];

        digits[10] = checksum(&digits[..10]).expect("ten PESEL payload digits");

        digits
            .iter()
            .map(|digit| char::from(b'0' + *digit))
            .collect()
    }

    #[test]
    fn accepts_valid_pesel_across_supported_centuries() {
        for year in [1899, 1999, 2000, 2100, 2200] {
            let pesel = pesel_for_date(year, 12, 31, [0, 3, 6, 2]);

            assert!(
                validate_pesel(&pesel).is_some(),
                "valid PESEL for year {year} must be accepted"
            );
        }
    }

    #[test]
    fn accepts_valid_leap_day() {
        let pesel = pesel_for_date(2000, 2, 29, [0, 3, 6, 2]);

        assert!(validate_pesel(&pesel).is_some());
    }

    #[test]
    fn accepts_either_sex_encoding() {
        let female = pesel_for_date(2002, 7, 8, [0, 3, 6, 2]);
        let male = pesel_for_date(2002, 7, 8, [0, 3, 6, 3]);

        assert!(validate_pesel(&female).is_some());
        assert!(validate_pesel(&male).is_some());
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(validate_pesel("0207080362").is_none());
        assert!(validate_pesel("020708036281").is_none());
    }

    #[test]
    fn rejects_non_ascii_digits() {
        assert!(validate_pesel("0207080362A").is_none());
    }

    #[test]
    fn rejects_invalid_encoded_month() {
        for encoded_month in [0, 13, 20, 33, 40, 53, 60, 73, 80, 93] {
            let source = format!("00{encoded_month:02}0103620");

            assert!(
                validate_pesel(&source).is_none(),
                "encoded month {encoded_month:02} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_impossible_calendar_dates() {
        for (year, month, day) in [
            (2001, 2, 29),
            (2000, 2, 30),
            (2000, 4, 31),
            (2000, 6, 31),
            (2000, 9, 31),
            (2000, 11, 31),
        ] {
            let pesel = pesel_for_date(year, month, day, [0, 3, 6, 2]);

            assert!(
                validate_pesel(&pesel).is_none(),
                "{year:04}-{month:02}-{day:02} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_day_zero() {
        let pesel = pesel_for_date(2000, 1, 0, [0, 3, 6, 2]);

        assert!(validate_pesel(&pesel).is_none());
    }

    #[test]
    fn rejects_wrong_checksum() {
        let mut pesel = pesel_for_date(2002, 7, 8, [0, 3, 6, 2]);
        let replacement = if pesel.ends_with('9') { '0' } else { '9' };

        pesel.pop();
        pesel.push(replacement);

        assert!(validate_pesel(&pesel).is_none());
    }
}
