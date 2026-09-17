//! Contextual validation for credentials embedded in Swift Package Manager's
//! `SWIFTPM_NETRC_DATA` environment variable.
//!
//! SwiftPM accepts inline `.netrc`-formatted credential data through this
//! variable. Candidate discovery identifies individual `password` values;
//! validation establishes both the SwiftPM container and the `.netrc`
//! `machine` → `login` → `password` record containing the candidate.

use super::{context::ValidationContext, netrc::validate_netrc_password};

const CONTEXT_WINDOW: usize = 16 * 1024;
const VARIABLE: &str = "SWIFTPM_NETRC_DATA";

/// Successful validation of a password embedded in `SWIFTPM_NETRC_DATA`.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct SwiftPmNetrcValidation;

/// Validates an individual `.netrc` password candidate embedded in
/// `SWIFTPM_NETRC_DATA`.
///
/// Container syntax belongs to SwiftPM rather than the generic `.netrc`
/// detector. The shared `.netrc` structural parser remains responsible for
/// establishing the credential record itself.
pub(crate) fn validate_swiftpm_netrc(
    context: &ValidationContext<'_>,
) -> Option<SwiftPmNetrcValidation> {
    let before = context.before_window(CONTEXT_WINDOW);
    let payload = swiftpm_netrc_payload_prefix(before)?;

    validate_netrc_password(context.candidate(), payload)?;

    Some(SwiftPmNetrcValidation)
}

/// Returns the portion of the inline `.netrc` payload preceding the candidate.
///
/// v0.4.5 deliberately supports quoted `SWIFTPM_NETRC_DATA` assignments.
/// The opening quote is container syntax and is excluded before the shared
/// `.netrc` parser sees the payload.
fn swiftpm_netrc_payload_prefix(before: &str) -> Option<&str> {
    let mut payload = None;

    for (start, _) in before.match_indices(VARIABLE) {
        let name_end = start + VARIABLE.len();

        if !is_assignment_name_boundary(before, start, name_end) {
            continue;
        }

        let tail = &before[name_end..];
        let tail = tail.trim_start();

        let Some(tail) = tail.strip_prefix('=') else {
            continue;
        };

        let tail = tail.trim_start();

        if let Some(value) = tail.strip_prefix('"').or_else(|| tail.strip_prefix('\'')) {
            payload = Some(value);
        }
    }

    payload
}

fn is_assignment_name_boundary(source: &str, start: usize, end: usize) -> bool {
    let bytes = source.as_bytes();

    let before = bytes[..start].last().copied();
    let after = bytes.get(end).copied();

    let valid_before =
        before.is_none_or(|byte| byte == b'\n' || byte == b'\r' || byte == b' ' || byte == b'\t');

    let valid_after = after.is_some_and(|byte| byte == b'=' || byte == b' ' || byte == b'\t');

    valid_before && valid_after
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_password_inside_quoted_swiftpm_netrc_data() {
        let password = "SwiftPMNetrcSecret_123456";
        let source = format!(
            r#"SWIFTPM_NETRC_DATA="machine registry.example.com login alice password {password}""#
        );

        assert!(validate_swiftpm_netrc(&context(&source, password)).is_some());
    }

    #[test]
    fn recognizes_later_machine_record_inside_multiline_payload() {
        let password = "SwiftPMNetrcSecretTwo_123456";
        let source = format!(
            "SWIFTPM_NETRC_DATA=\"machine first.example.com login alice password FirstSecret_123456\n\
             machine second.example.com login bob password {password}\""
        );

        assert!(validate_swiftpm_netrc(&context(&source, password)).is_some());
    }

    #[test]
    fn rejects_netrc_record_outside_swiftpm_container() {
        let password = "SwiftPMNetrcSecret_123456";
        let source = format!("machine registry.example.com login alice password {password}");

        assert!(validate_swiftpm_netrc(&context(&source, password)).is_none());
    }

    #[test]
    fn rejects_incomplete_record_inside_swiftpm_container() {
        let password = "SwiftPMNetrcSecret_123456";
        let source = format!(r#"SWIFTPM_NETRC_DATA="password {password}""#);

        assert!(validate_swiftpm_netrc(&context(&source, password)).is_none());
    }

    #[test]
    fn rejects_placeholder_password() {
        let source =
            r#"SWIFTPM_NETRC_DATA="machine registry.example.com login alice password changeme""#;

        let start = source.find("changeme").expect("fixture");
        let context = ValidationContext::new(source, start..start + "changeme".len());

        assert!(validate_swiftpm_netrc(&context).is_none());
    }

    #[test]
    fn rejects_prefixed_container_name() {
        let password = "SwiftPMNetrcSecret_123456";
        let source = format!(
            r#"MY_SWIFTPM_NETRC_DATA="machine registry.example.com login alice password {password}""#
        );

        assert!(validate_swiftpm_netrc(&context(&source, password)).is_none());
    }

    #[test]
    fn rejects_suffixed_container_name() {
        let password = "SwiftPMNetrcSecret_123456";
        let source = format!(
            r#"SWIFTPM_NETRC_DATA_SUFFIX="machine registry.example.com login alice password {password}""#
        );

        assert!(validate_swiftpm_netrc(&context(&source, password)).is_none());
    }
}
