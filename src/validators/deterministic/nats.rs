//! Structural validation for NATS NKey private material.
//!
//! NKey seeds and encoded private keys are Base32 representations carrying
//! type information and a CRC16-XMODEM checksum. Public NKeys are deliberately
//! not classified because they are public authentication identities rather
//! than secret material.

use crate::validators::utils::is_obvious_placeholder;

const PREFIX_BYTE_SEED: u8 = 18 << 3;
const PREFIX_BYTE_PRIVATE: u8 = 15 << 3;

const PREFIX_BYTE_SERVER: u8 = 13 << 3;
const PREFIX_BYTE_CLUSTER: u8 = 2 << 3;
const PREFIX_BYTE_OPERATOR: u8 = 14 << 3;
const PREFIX_BYTE_ACCOUNT: u8 = 0;
const PREFIX_BYTE_USER: u8 = 20 << 3;
const PREFIX_BYTE_CURVE: u8 = 23 << 3;

const SEED_LEN: usize = 32;
const PRIVATE_KEY_LEN: usize = 64;

const ENCODED_SEED_LEN: usize = 58;
const ENCODED_PRIVATE_KEY_LEN: usize = 108;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum NatsNkeyKind {
    Seed,
    PrivateKey,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct NatsNkeyValidation {
    kind: NatsNkeyKind,
}

impl NatsNkeyValidation {
    pub(crate) const fn kind(self) -> NatsNkeyKind {
        self.kind
    }
}

pub(crate) fn validate_nats_nkey(candidate: &str) -> Option<NatsNkeyValidation> {
    if !candidate.is_ascii() || is_obvious_placeholder(candidate) {
        return None;
    }

    match candidate.len() {
        ENCODED_SEED_LEN => validate_seed(candidate),
        ENCODED_PRIVATE_KEY_LEN => validate_private_key(candidate),
        _ => None,
    }
}

fn validate_seed(candidate: &str) -> Option<NatsNkeyValidation> {
    let raw = decode_base32(candidate)?;

    if raw.len() != 2 + SEED_LEN + 2 || !valid_crc(&raw) {
        return None;
    }

    let first = raw[0];
    let second = raw[1];

    if first & 0xf8 != PREFIX_BYTE_SEED {
        return None;
    }

    let public_prefix = (first & 0x07) << 5 | ((second & 0xf8) >> 3);

    if !matches!(
        public_prefix,
        PREFIX_BYTE_SERVER
            | PREFIX_BYTE_CLUSTER
            | PREFIX_BYTE_OPERATOR
            | PREFIX_BYTE_ACCOUNT
            | PREFIX_BYTE_USER
            | PREFIX_BYTE_CURVE
    ) {
        return None;
    }

    Some(NatsNkeyValidation {
        kind: NatsNkeyKind::Seed,
    })
}

fn validate_private_key(candidate: &str) -> Option<NatsNkeyValidation> {
    let raw = decode_base32(candidate)?;

    if raw.len() != 1 + PRIVATE_KEY_LEN + 2
        || raw[0] & 0xf8 != PREFIX_BYTE_PRIVATE
        || !valid_crc(&raw)
    {
        return None;
    }

    Some(NatsNkeyValidation {
        kind: NatsNkeyKind::PrivateKey,
    })
}

fn valid_crc(raw: &[u8]) -> bool {
    let Some(payload_len) = raw.len().checked_sub(2) else {
        return false;
    };

    let expected = u16::from_le_bytes([raw[payload_len], raw[payload_len + 1]]);

    crc16_xmodem(&raw[..payload_len]) == expected
}

fn crc16_xmodem(bytes: &[u8]) -> u16 {
    let mut crc = 0_u16;

    for &byte in bytes {
        crc ^= u16::from(byte) << 8;

        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }

    crc
}

fn decode_base32(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(value.len() * 5 / 8);
    let mut accumulator = 0_u32;
    let mut bits = 0_u8;

    for byte in value.bytes() {
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'2'..=b'7' => byte - b'2' + 26,
            _ => return None,
        };

        accumulator = (accumulator << 5) | u32::from(digit);
        bits += 5;

        if bits >= 8 {
            bits -= 8;
            output.push((accumulator >> bits) as u8);

            if bits == 0 {
                accumulator = 0;
            } else {
                accumulator &= (1_u32 << bits) - 1;
            }
        }
    }

    if bits != 0 && accumulator != 0 {
        return None;
    }

    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_base32(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

        let mut output = String::new();
        let mut accumulator = 0_u32;
        let mut bits = 0_u8;

        for &byte in bytes {
            accumulator = (accumulator << 8) | u32::from(byte);
            bits += 8;

            while bits >= 5 {
                bits -= 5;
                output.push(ALPHABET[((accumulator >> bits) & 0x1f) as usize] as char);

                if bits == 0 {
                    accumulator = 0;
                } else {
                    accumulator &= (1_u32 << bits) - 1;
                }
            }
        }

        if bits != 0 {
            output.push(ALPHABET[((accumulator << (5 - bits)) & 0x1f) as usize] as char);
        }

        output
    }

    fn with_crc(mut bytes: Vec<u8>) -> String {
        bytes.extend_from_slice(&crc16_xmodem(&bytes).to_le_bytes());
        encode_base32(&bytes)
    }

    fn seed(public_prefix: u8) -> String {
        let first = PREFIX_BYTE_SEED | (public_prefix >> 5);
        let second = (public_prefix & 31) << 3;

        let mut raw = vec![first, second];
        raw.extend_from_slice(&[0x42; SEED_LEN]);
        with_crc(raw)
    }

    fn private_key() -> String {
        let mut raw = vec![PREFIX_BYTE_PRIVATE];
        raw.extend_from_slice(&[0x24; PRIVATE_KEY_LEN]);
        with_crc(raw)
    }

    #[test]
    fn accepts_supported_seed_types() {
        for prefix in [
            PREFIX_BYTE_OPERATOR,
            PREFIX_BYTE_SERVER,
            PREFIX_BYTE_CLUSTER,
            PREFIX_BYTE_ACCOUNT,
            PREFIX_BYTE_USER,
            PREFIX_BYTE_CURVE,
        ] {
            let candidate = seed(prefix);

            assert_eq!(candidate.len(), ENCODED_SEED_LEN);
            assert_eq!(
                validate_nats_nkey(&candidate).map(NatsNkeyValidation::kind),
                Some(NatsNkeyKind::Seed),
            );
        }
    }

    #[test]
    fn accepts_encoded_private_key() {
        let candidate = private_key();

        assert_eq!(candidate.len(), ENCODED_PRIVATE_KEY_LEN);
        assert_eq!(
            validate_nats_nkey(&candidate).map(NatsNkeyValidation::kind),
            Some(NatsNkeyKind::PrivateKey),
        );
    }

    #[test]
    fn rejects_invalid_checksum() {
        let mut candidate = seed(PREFIX_BYTE_USER).into_bytes();
        let last = candidate.len() - 1;
        candidate[last] = if candidate[last] == b'A' { b'B' } else { b'A' };

        assert!(validate_nats_nkey(std::str::from_utf8(&candidate).unwrap()).is_none());
    }

    #[test]
    fn rejects_invalid_base32_and_case_changed_values() {
        let candidate = seed(PREFIX_BYTE_USER);

        let mut invalid = candidate.clone();
        invalid.replace_range(10..11, "0");
        assert!(validate_nats_nkey(&invalid).is_none());

        assert!(validate_nats_nkey(&candidate.to_ascii_lowercase()).is_none());
    }

    #[test]
    fn rejects_public_nkeys() {
        let mut raw = vec![PREFIX_BYTE_USER];
        raw.extend_from_slice(&[0x42; 32]);

        assert!(validate_nats_nkey(&with_crc(raw)).is_none());
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(validate_nats_nkey("SUABCDEF").is_none());
    }
}
