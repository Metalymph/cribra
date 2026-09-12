//! Contextual validation for system password verifiers.
//!
//! Password verifier strings are not secrets merely because they use a modular
//! crypt syntax. Validation requires a recognized system authentication record
//! and a supported verifier family.

use super::context::ValidationContext;

const MAX_VERIFIER_LEN: usize = 1024;

/// Supported system password verifier families.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum SystemPasswordVerifierKind {
    Apr1,
    Bcrypt,
    Sha256Crypt,
    Sha512Crypt,
    Yescrypt,
}

/// Successful system password verifier classification.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct SystemPasswordVerifierValidation {
    kind: SystemPasswordVerifierKind,
}

impl SystemPasswordVerifierValidation {
    pub(crate) const fn kind(self) -> SystemPasswordVerifierKind {
        self.kind
    }
}

/// Validates a supported password verifier occupying the password field of an
/// `/etc/shadow`-style record.
///
/// Cribra deliberately requires record structure here. A standalone `$6$...`
/// or `$y$...` string is not sufficient evidence of sensitive authentication
/// material.
pub(crate) fn validate_system_password_verifier(
    context: &ValidationContext<'_>,
) -> Option<SystemPasswordVerifierValidation> {
    let candidate = context.candidate();

    if candidate.is_empty()
        || candidate.len() > MAX_VERIFIER_LEN
        || candidate.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return None;
    }

    let kind = classify_shadow_verifier(candidate)?;

    is_shadow_password_field(context).then_some(SystemPasswordVerifierValidation { kind })
}

fn classify_shadow_verifier(candidate: &str) -> Option<SystemPasswordVerifierKind> {
    if candidate.starts_with("$5$") {
        Some(SystemPasswordVerifierKind::Sha256Crypt)
    } else if candidate.starts_with("$6$") {
        Some(SystemPasswordVerifierKind::Sha512Crypt)
    } else if candidate.starts_with("$y$") {
        Some(SystemPasswordVerifierKind::Yescrypt)
    } else {
        None
    }
}

/// Validates a supported password verifier occupying the verifier field of an
/// Apache `.htpasswd`-style `username:verifier` record.
pub(crate) fn validate_htpasswd_verifier(
    context: &ValidationContext<'_>,
) -> Option<SystemPasswordVerifierValidation> {
    let candidate = context.candidate();

    if candidate.is_empty()
        || candidate.len() > MAX_VERIFIER_LEN
        || candidate.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return None;
    }

    let kind = classify_htpasswd_verifier(candidate)?;

    is_htpasswd_verifier_field(context).then_some(SystemPasswordVerifierValidation { kind })
}

fn is_htpasswd_verifier_field(context: &ValidationContext<'_>) -> bool {
    const LINE_WINDOW: usize = 4096;

    let before = context.before_window(LINE_WINDOW);
    let after = context.after_window(LINE_WINDOW);

    let prefix = before.rsplit_once('\n').map_or(before, |(_, line)| line);
    let suffix = after.split_once('\n').map_or(after, |(line, _)| line);

    let Some(username) = prefix.strip_suffix(':') else {
        return false;
    };

    !username.is_empty()
        && !username.contains(':')
        && !username.bytes().any(|byte| byte.is_ascii_whitespace())
        && suffix.is_empty()
}

fn classify_htpasswd_verifier(candidate: &str) -> Option<SystemPasswordVerifierKind> {
    if is_apr1(candidate) {
        Some(SystemPasswordVerifierKind::Apr1)
    } else if is_bcrypt(candidate) {
        Some(SystemPasswordVerifierKind::Bcrypt)
    } else {
        None
    }
}

fn is_apr1(candidate: &str) -> bool {
    let Some(rest) = candidate.strip_prefix("$apr1$") else {
        return false;
    };

    let Some((salt, digest)) = rest.split_once('$') else {
        return false;
    };

    !salt.is_empty()
        && salt.len() <= 8
        && salt.bytes().all(is_crypt_base64)
        && digest.len() == 22
        && digest.bytes().all(is_crypt_base64)
}

fn is_bcrypt(candidate: &str) -> bool {
    if candidate.len() != 60 {
        return false;
    }

    let bytes = candidate.as_bytes();

    if !matches!(&bytes[..4], b"$2a$" | b"$2b$" | b"$2y$") {
        return false;
    }

    let cost = &candidate[4..6];

    if !cost.bytes().all(|byte| byte.is_ascii_digit()) || bytes[6] != b'$' {
        return false;
    }

    let Ok(cost) = cost.parse::<u8>() else {
        return false;
    };

    (4..=31).contains(&cost)
        && candidate[7..]
            .bytes()
            .all(|byte| matches!(byte, b'.' | b'/' | b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'))
}

fn is_crypt_base64(byte: u8) -> bool {
    matches!(
        byte,
        b'.' | b'/' | b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
    )
}

/// Establishes that the candidate is the second colon-delimited field of a
/// shadow-style record.
///
/// Requiring the conventional trailing shadow fields prevents arbitrary
/// `name:$6$...` text from being promoted merely because it contains a modular
/// crypt verifier.
fn is_shadow_password_field(context: &ValidationContext<'_>) -> bool {
    const LINE_WINDOW: usize = 4096;

    let before = context.before_window(LINE_WINDOW);
    let after = context.after_window(LINE_WINDOW);

    let line_prefix = before.rsplit_once('\n').map_or(before, |(_, line)| line);
    let line_suffix = after.split_once('\n').map_or(after, |(line, _)| line);

    // Candidate must be exactly the second colon-delimited field.
    let Some(username) = line_prefix.strip_suffix(':') else {
        return false;
    };

    if username.is_empty()
        || username.contains(':')
        || username.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return false;
    }

    let Some(rest) = line_suffix.strip_prefix(':') else {
        return false;
    };

    let fields = rest.split(':').collect::<Vec<_>>();

    // Seven fields follow the password field in a conventional shadow record:
    // lastchg:min:max:warn:inactive:expire:reserved
    if fields.len() != 7 {
        return false;
    }

    // Shadow aging fields are either empty or decimal integers.
    fields
        .iter()
        .all(|field| field.is_empty() || field.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_htpasswd_apr1() {
        let verifier = "$apr1$hfT7jp2q$2VbDVlM1QY3wP1uQYxJYB/";
        let source = format!("alice:{verifier}");

        assert_eq!(
            validate_htpasswd_verifier(&context(&source, verifier))
                .map(SystemPasswordVerifierValidation::kind),
            Some(SystemPasswordVerifierKind::Apr1),
        );
    }

    #[test]
    fn recognizes_htpasswd_bcrypt() {
        let verifier = "$2y$12$123456789012345678901u1234567890123456789012345678901";
        let source = format!("alice:{verifier}");

        assert_eq!(
            validate_htpasswd_verifier(&context(&source, verifier))
                .map(SystemPasswordVerifierValidation::kind),
            Some(SystemPasswordVerifierKind::Bcrypt),
        );
    }
}
