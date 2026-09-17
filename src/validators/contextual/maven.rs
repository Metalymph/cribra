//! Contextual validation for Maven repository/server credentials.
//!
//! Maven server passwords are accepted only when the candidate belongs to a
//! bounded `<servers><server>...</server></servers>` region. Recognizable Maven
//! encrypted/protected password representations are deliberately rejected.

use super::context::ValidationContext;
use crate::validators::utils::is_obvious_placeholder;

const CONTEXT_WINDOW: usize = 4096;
const MAX_PASSWORD_LEN: usize = 2048;

/// Successful Maven server-password validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct MavenValidation;

/// Validates a cleartext Maven `<server>` password.
pub(crate) fn validate_maven(context: &ValidationContext<'_>) -> Option<MavenValidation> {
    let candidate = context.candidate();

    if candidate.is_empty()
        || candidate.len() > MAX_PASSWORD_LEN
        || candidate.chars().any(char::is_control)
        || is_obvious_placeholder(candidate)
        || is_maven_documentation_password(candidate)
        || looks_like_maven_protected_password(candidate)
    {
        return None;
    }

    let before = context.before_window(CONTEXT_WINDOW);
    let after = context.after_window(CONTEXT_WINDOW);

    if !inside_maven_server(before, after) {
        return None;
    }

    Some(MavenValidation)
}

fn is_maven_documentation_password(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "password" | "your_password" | "your_password_here" | "example_password" | "maven_password"
    )
}

fn looks_like_maven_protected_password(value: &str) -> bool {
    let value = value.trim();

    // Maven 3 legacy encrypted server/master-password representation.
    //
    // Maven 4 also uses dispatcher-enveloped forms beginning with `{[` and
    // ending in `}`. Both are protected representations and must not be
    // classified as cleartext Maven passwords.
    value.starts_with('{') && value.ends_with('}')
}

fn inside_maven_server(before: &str, after: &str) -> bool {
    let Some(servers_start) = rfind_ascii_case_insensitive(before, "<servers") else {
        return false;
    };

    let before_servers = &before[servers_start..];

    // A completed </servers> after the last opening section means the
    // candidate is outside that credentials region.
    if contains_ascii_case_insensitive(before_servers, "</servers") {
        return false;
    }

    let Some(server_start) = rfind_ascii_case_insensitive(before_servers, "<server") else {
        return false;
    };

    let before_server = &before_servers[server_start..];

    // Do not allow context from an already completed sibling <server>.
    if contains_ascii_case_insensitive(before_server, "</server") {
        return false;
    }

    contains_ascii_case_insensitive(after, "</server")
        && contains_ascii_case_insensitive(after, "</servers")
}

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }

    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn rfind_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }

    haystack
        .as_bytes()
        .windows(needle.len())
        .rposition(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_cleartext_password_inside_server_credentials() {
        let password = "CorrectHorseBatteryStaple";
        let source = format!(
            "<settings><servers><server><id>private</id><username>alice</username>\
             <password>{password}</password></server></servers></settings>"
        );

        assert!(validate_maven(&context(&source, password)).is_some());
    }

    #[test]
    fn rejects_password_outside_servers() {
        let password = "CorrectHorseBatteryStaple";
        let source = format!("<configuration><password>{password}</password></configuration>");

        assert!(validate_maven(&context(&source, password)).is_none());
    }

    #[test]
    fn rejects_legacy_encrypted_password() {
        let password = "{COQLCE6DU6GtcS5P=}";
        let source = format!(
            "<settings><servers><server><id>private</id>\
             <password>{password}</password></server></servers></settings>"
        );

        assert!(validate_maven(&context(&source, password)).is_none());
    }

    #[test]
    fn rejects_maven4_protected_password() {
        let password = "{[name=master,version=4.0]synthetic-value}";
        let source = format!(
            "<settings><servers><server><id>private</id>\
             <password>{password}</password></server></servers></settings>"
        );

        assert!(validate_maven(&context(&source, password)).is_none());
    }

    #[test]
    fn rejects_placeholders() {
        let password = "your_password_here";
        let source = format!(
            "<settings><servers><server><id>private</id>\
             <password>{password}</password></server></servers></settings>"
        );

        assert!(validate_maven(&context(&source, password)).is_none());
    }
}
