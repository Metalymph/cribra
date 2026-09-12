//! Contextual validation for WireGuard private and preshared keys.
//!
//! WireGuard keys have a strong structural shape, but the same Base64 shape can
//! occur in unrelated data. Detection therefore requires both a valid 32-byte
//! key and the appropriate WireGuard configuration section.

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{
    context::ValidationContext,
    utils::{key_matches_any, nearest_key},
};

const CONTEXT_WINDOW: usize = 8192;
const ENCODED_KEY_LEN: usize = 44;
const DECODED_KEY_LEN: usize = 32;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum WireGuardCredentialKind {
    PrivateKey,
    PresharedKey,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct WireGuardValidation {
    kind: WireGuardCredentialKind,
}

impl WireGuardValidation {
    pub(crate) const fn kind(self) -> WireGuardCredentialKind {
        self.kind
    }
}

pub(crate) fn validate_wireguard(context: &ValidationContext<'_>) -> Option<WireGuardValidation> {
    let candidate = context.candidate();

    if candidate.len() != ENCODED_KEY_LEN || !candidate.is_ascii() {
        return None;
    }

    let decoded = STANDARD.decode(candidate).ok()?;

    if decoded.len() != DECODED_KEY_LEN {
        return None;
    }

    // Require canonical standard Base64 rather than accepting alternate or
    // malformed encodings that happen to decode successfully.
    if STANDARD.encode(&decoded) != candidate {
        return None;
    }

    // An all-zero value is a common placeholder/invalid configuration value,
    // not useful secret material.
    if decoded.iter().all(|byte| *byte == 0) {
        return None;
    }

    let before = context.before_window(CONTEXT_WINDOW);
    let key = nearest_key(before)?;

    let kind = if key_matches_any(key, &["privatekey"]) {
        if current_wireguard_section(before)? != WireGuardSection::Interface {
            return None;
        }

        WireGuardCredentialKind::PrivateKey
    } else if key_matches_any(key, &["presharedkey"]) {
        if current_wireguard_section(before)? != WireGuardSection::Peer {
            return None;
        }

        WireGuardCredentialKind::PresharedKey
    } else {
        return None;
    };

    Some(WireGuardValidation { kind })
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum WireGuardSection {
    Interface,
    Peer,
}

fn current_wireguard_section(before: &str) -> Option<WireGuardSection> {
    for line in before.lines().rev() {
        let line = line.trim();

        if line.eq_ignore_ascii_case("[interface]") {
            return Some(WireGuardSection::Interface);
        }

        if line.eq_ignore_ascii_case("[peer]") {
            return Some(WireGuardSection::Peer);
        }

        // A different INI-style section terminates WireGuard context. This
        // prevents an earlier WireGuard header from authorizing an unrelated
        // section later in the source.
        if line.starts_with('[') && line.ends_with(']') {
            return None;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_private_and_preshared_keys_in_correct_sections() {
        let private_key = STANDARD.encode([0x11; DECODED_KEY_LEN]);
        let private_source =
            format!("[Interface]\nAddress = 10.0.0.2/32\nPrivateKey = {private_key}");

        assert_eq!(
            validate_wireguard(&context(&private_source, &private_key))
                .map(WireGuardValidation::kind),
            Some(WireGuardCredentialKind::PrivateKey),
        );

        let preshared_key = STANDARD.encode([0x22; DECODED_KEY_LEN]);
        let preshared_source = format!(
            "[Peer]\nPublicKey = {}\nPresharedKey = {preshared_key}",
            STANDARD.encode([0x33; DECODED_KEY_LEN]),
        );

        assert_eq!(
            validate_wireguard(&context(&preshared_source, &preshared_key))
                .map(WireGuardValidation::kind),
            Some(WireGuardCredentialKind::PresharedKey),
        );
    }

    #[test]
    fn rejects_keys_outside_the_required_wireguard_section() {
        let value = STANDARD.encode([0x11; DECODED_KEY_LEN]);

        for source in [
            format!("PrivateKey = {value}"),
            format!("[Peer]\nPrivateKey = {value}"),
            format!("[Interface]\nPresharedKey = {value}"),
            format!("[Other]\nPrivateKey = {value}"),
            format!("[Interface]\n[Other]\nPrivateKey = {value}"),
        ] {
            assert!(
                validate_wireguard(&context(&source, &value)).is_none(),
                "unexpected WireGuard validation for {source:?}",
            );
        }
    }

    #[test]
    fn rejects_invalid_key_structure_and_zero_placeholder() {
        let zero_key = STANDARD.encode([0_u8; DECODED_KEY_LEN]);
        let source = format!("[Interface]\nPrivateKey = {zero_key}");

        assert!(validate_wireguard(&context(&source, &zero_key)).is_none());

        for value in [
            "not-base64",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ] {
            let source = format!("[Interface]\nPrivateKey = {value}");

            assert!(validate_wireguard(&context(&source, value)).is_none());
        }
    }
}
