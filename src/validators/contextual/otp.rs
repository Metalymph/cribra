//! Contextual validation for TOTP/HOTP shared provisioning secrets.
//!
//! Base32 data alone is not sufficient evidence of an OTP credential.
//! Validation requires either an `otpauth` provisioning URI or an explicit
//! OTP-secret configuration key.

use super::{
    context::ValidationContext,
    utils::{DEFAULT_KEY_WINDOW, key_matches_any, nearest_key},
};

use crate::validators::utils::is_obvious_placeholder;

const OTP_SECRET_KEYS: &[&str] = &["otp_secret", "totp_secret", "hotp_secret"];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct OtpProvisioningSecretValidation;

/// Validates a TOTP/HOTP shared provisioning secret in explicit OTP context.
pub(crate) fn validate_otp_provisioning_secret(
    context: &ValidationContext<'_>,
) -> Option<OtpProvisioningSecretValidation> {
    let candidate = context.candidate();

    if is_obvious_placeholder(candidate) || !valid_base32_secret(candidate) {
        return None;
    }

    if has_explicit_otp_key(context) || has_otpauth_context(context) {
        return Some(OtpProvisioningSecretValidation);
    }

    None
}

fn has_explicit_otp_key(context: &ValidationContext<'_>) -> bool {
    nearest_key(context.before_window(DEFAULT_KEY_WINDOW))
        .is_some_and(|key| key_matches_any(key, OTP_SECRET_KEYS))
}

fn has_otpauth_context(context: &ValidationContext<'_>) -> bool {
    let before = context.before_window(DEFAULT_KEY_WINDOW);
    let lower = before.to_ascii_lowercase();

    let Some(scheme) = lower.rfind("otpauth://") else {
        return false;
    };

    let uri = &lower[scheme..];

    let valid_kind = uri.starts_with("otpauth://totp/") || uri.starts_with("otpauth://hotp/");

    if !valid_kind {
        return false;
    }

    // The candidate must belong specifically to the `secret` query parameter.
    let Some(secret) = uri.rfind("secret=") else {
        return false;
    };

    let prefix = &uri[..secret];

    prefix.ends_with('?') || prefix.ends_with('&')
}

fn valid_base32_secret(value: &str) -> bool {
    // 16 Base32 characters represent 80 bits, a conservative minimum for a
    // provisioning secret and enough to reject short identifier-like values.
    if value.len() < 16 || !value.is_ascii() {
        return false;
    }

    let unpadded_len = value.trim_end_matches('=').len();
    let (data, padding) = value.split_at(unpadded_len);

    if data.is_empty()
        || !data
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'2'..=b'7'))
        || !padding.bytes().all(|byte| byte == b'=')
    {
        return false;
    }

    // RFC 4648 Base32 permits these remainder/padding combinations.
    matches!(
        (data.len() % 8, padding.len()),
        (0, 0) | (2, 6) | (4, 4) | (5, 3) | (7, 1)
    )
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
    fn accepts_totp_secret_in_explicit_configuration_context() {
        let secret = "JBSWY3DPEHPK3PXP";

        for key in ["otp_secret", "totp_secret", "totp-secret", "totp.secret"] {
            let source = format!("{key}={secret}");

            assert!(
                validate_otp_provisioning_secret(&context(&source, secret)).is_some(),
                "expected OTP secret context for key {key:?}",
            );
        }
    }

    #[test]
    fn accepts_hotp_secret_in_explicit_configuration_context() {
        let secret = "JBSWY3DPEHPK3PXP";
        let source = format!("hotp_secret={secret}");

        assert!(validate_otp_provisioning_secret(&context(&source, secret)).is_some());
    }

    #[test]
    fn accepts_secret_in_totp_otpauth_uri() {
        let secret = "JBSWY3DPEHPK3PXP";
        let source =
            format!("otpauth://totp/example:user@example.com?secret={secret}&issuer=Example");

        assert!(validate_otp_provisioning_secret(&context(&source, secret)).is_some());
    }

    #[test]
    fn accepts_secret_in_hotp_otpauth_uri() {
        let secret = "JBSWY3DPEHPK3PXP";
        let source = format!("otpauth://hotp/example?counter=0&secret={secret}&issuer=Example");

        assert!(validate_otp_provisioning_secret(&context(&source, secret)).is_some());
    }

    #[test]
    fn accepts_secret_as_first_or_later_otpauth_query_parameter() {
        let secret = "JBSWY3DPEHPK3PXP";

        for source in [
            format!("otpauth://totp/example?secret={secret}&issuer=Example"),
            format!("otpauth://totp/example?issuer=Example&secret={secret}"),
        ] {
            assert!(
                validate_otp_provisioning_secret(&context(&source, secret)).is_some(),
                "expected provisioning URI to validate: {source:?}",
            );
        }
    }

    #[test]
    fn accepts_lowercase_base32_in_authoritative_context() {
        let secret = "jbswy3dpehpk3pxp";
        let source = format!("totp_secret={secret}");

        assert!(validate_otp_provisioning_secret(&context(&source, secret)).is_some());
    }

    #[test]
    fn accepts_valid_canonical_base32_padding() {
        let secret = "ABCDEFGHIJKLMNOPQR======";
        let source = format!("otp_secret={secret}");

        assert!(validate_otp_provisioning_secret(&context(&source, secret)).is_some());
    }

    #[test]
    fn rejects_bare_base32_secret() {
        let secret = "JBSWY3DPEHPK3PXP";

        assert!(validate_otp_provisioning_secret(&context(secret, secret)).is_none());
    }

    #[test]
    fn rejects_generic_secret_context() {
        let secret = "JBSWY3DPEHPK3PXP";

        for key in ["secret", "api_key", "token", "password"] {
            let source = format!("{key}={secret}");

            assert!(
                validate_otp_provisioning_secret(&context(&source, secret)).is_none(),
                "generic key {key:?} must not establish OTP ownership",
            );
        }
    }

    #[test]
    fn rejects_non_otp_otpauth_scheme_or_type() {
        let secret = "JBSWY3DPEHPK3PXP";

        for source in [
            format!("https://example.com/?secret={secret}"),
            format!("otpauth://example/account?secret={secret}"),
        ] {
            assert!(
                validate_otp_provisioning_secret(&context(&source, secret)).is_none(),
                "unexpected OTP validation for {source:?}",
            );
        }
    }

    #[test]
    fn rejects_unrelated_otpauth_query_parameter() {
        let secret = "JBSWY3DPEHPK3PXP";
        let source = format!("otpauth://totp/example?issuer={secret}");

        assert!(validate_otp_provisioning_secret(&context(&source, secret)).is_none());
    }

    #[test]
    fn rejects_short_or_invalid_base32() {
        for secret in [
            "JBSWY3DP",
            "JBSWY3DPEHPK3PX0",
            "JBSWY3DPEHPK3PX1",
            "JBSWY3DPEHPK3PX8",
            "JBSWY3DPEHPK3PX!",
        ] {
            let source = format!("totp_secret={secret}");

            assert!(
                validate_otp_provisioning_secret(&context(&source, secret)).is_none(),
                "unexpected acceptance for {secret:?}",
            );
        }
    }

    #[test]
    fn rejects_invalid_base32_padding() {
        for secret in [
            "ABCDEFGHIJKLMNOP=",
            "ABCDEFGHIJKLMNOP===",
            "ABCDEFGHIJKLMNOP====",
            "ABCDEFGHIJKLMNOP=======",
            "ABC=DEFGHIJKLMNOP",
        ] {
            let source = format!("otp_secret={secret}");

            assert!(
                validate_otp_provisioning_secret(&context(&source, secret)).is_none(),
                "unexpected acceptance for invalid padding {secret:?}",
            );
        }
    }

    #[test]
    fn rejects_obvious_placeholders_in_authoritative_context() {
        for secret in [
            "xxxxxxxxxxxxxxxx",
            "XXXXXXXXXXXXXXXX",
            "aaaaaaaaaaaaaaaa",
            "AAAAAAAAAAAAAAAA",
        ] {
            let source = format!("otp_secret={secret}");

            assert!(
                validate_otp_provisioning_secret(&context(&source, secret)).is_none(),
                "placeholder OTP secret unexpectedly validated: {secret:?}",
            );
        }
    }
}
