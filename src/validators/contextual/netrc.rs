//! Contextual validation for `.netrc` machine credentials.
//!
//! `.netrc` credentials are not identifiable from the password value alone.
//! Validation therefore requires a narrow record structure containing a
//! `machine` entry followed by a `login` and the candidate `password`.
//!
//! Cribra intentionally implements only the credential-bearing subset needed
//! for reliable detection. It does not attempt to become a complete `.netrc`
//! parser.

use super::context::ValidationContext;
use crate::validators::utils::is_obvious_placeholder;

const CONTEXT_WINDOW: usize = 16 * 1024;
const MAX_PASSWORD_LEN: usize = 1024;

/// Successful `.netrc` credential validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct NetrcValidation;

/// Validates a password candidate belonging to a complete `.netrc`
/// `machine` → `login` → `password` record.
///
/// The validator accepts both single-line and multiline layouts because
/// `.netrc` fields are whitespace-delimited. A new `machine`, `default`, or
/// `macdef` token terminates the previous credential record.
pub(crate) fn validate_netrc(context: &ValidationContext<'_>) -> Option<NetrcValidation> {
    let candidate = context.candidate();

    if candidate.is_empty()
        || candidate.len() > MAX_PASSWORD_LEN
        || candidate.chars().any(char::is_whitespace)
        || candidate.chars().any(char::is_control)
        || is_obvious_placeholder(candidate)
        || is_documentation_password(candidate)
    {
        return None;
    }

    let before = context.before_window(CONTEXT_WINDOW);

    has_complete_machine_record(before).then_some(NetrcValidation)
}

/// Recognizes the narrow `.netrc` record prefix immediately preceding a
/// password candidate.
///
/// Comments are ignored from `#` to end-of-line. The parser deliberately
/// recognizes only the fields needed to establish credential context rather
/// than accepting arbitrary text containing the same words.
fn has_complete_machine_record(before: &str) -> bool {
    let mut machine = false;
    let mut login = false;
    let mut password_keyword = false;

    let mut expect_machine_name = false;
    let mut expect_login_value = false;

    for line in before.lines() {
        let line = line.split_once('#').map_or(line, |(content, _)| content);

        for token in line.split_ascii_whitespace() {
            if expect_machine_name {
                machine = !token.is_empty();
                login = false;
                password_keyword = false;
                expect_machine_name = false;
                continue;
            }

            if expect_login_value {
                login = machine && !token.is_empty();
                password_keyword = false;
                expect_login_value = false;
                continue;
            }

            match token {
                "machine" => {
                    machine = false;
                    login = false;
                    password_keyword = false;
                    expect_machine_name = true;
                }

                // These begin a different `.netrc` record or grammar surface.
                // The v0.4.3 detector intentionally does not infer credentials
                // across them.
                "default" | "macdef" => {
                    machine = false;
                    login = false;
                    password_keyword = false;
                    expect_machine_name = false;
                    expect_login_value = false;
                }

                "login" if machine => {
                    login = false;
                    password_keyword = false;
                    expect_login_value = true;
                }

                "password" if machine && login => {
                    password_keyword = true;
                }

                _ => {}
            }
        }
    }

    machine && login && password_keyword && !expect_machine_name && !expect_login_value
}

fn is_documentation_password(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "password"
            | "passwd"
            | "your_password"
            | "your_password_here"
            | "example_password"
            | "replace_me"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context<'a>(source: &'a str, value: &'a str) -> ValidationContext<'a> {
        let start = source.find(value).expect("fixture must contain value");
        ValidationContext::new(source, start..start + value.len())
    }

    #[test]
    fn recognizes_single_line_machine_record() {
        let password = "CorrectHorseBatteryStaple";
        let source = format!("machine registry.example.com login alice password {password}");

        assert!(validate_netrc(&context(&source, password)).is_some());
    }

    #[test]
    fn recognizes_multiline_machine_record() {
        let password = "CorrectHorseBatteryStaple";
        let source = format!(
            "machine registry.example.com\n\
             login alice\n\
             password {password}\n"
        );

        assert!(validate_netrc(&context(&source, password)).is_some());
    }

    #[test]
    fn accepts_common_mixed_whitespace_layout() {
        let password = "CorrectHorseBatteryStaple";
        let source = format!("machine registry.example.com\tlogin alice\n\tpassword {password}");

        assert!(validate_netrc(&context(&source, password)).is_some());
    }

    #[test]
    fn rejects_incomplete_machine_records() {
        let password = "CorrectHorseBatteryStaple";

        for source in [
            format!("password {password}"),
            format!("machine registry.example.com password {password}"),
            format!("login alice password {password}"),
            format!("machine registry.example.com login password {password}"),
        ] {
            assert!(
                validate_netrc(&context(&source, password)).is_none(),
                "unexpected .netrc validation for incomplete fixture",
            );
        }
    }

    #[test]
    fn does_not_cross_record_boundaries() {
        let password = "CorrectHorseBatteryStaple";

        for source in [
            format!(
                "machine first.example.com login alice\n\
                 machine second.example.com password {password}"
            ),
            format!(
                "machine first.example.com login alice\n\
                 default password {password}"
            ),
            format!(
                "machine first.example.com login alice\n\
                 macdef init\n\
                 password {password}"
            ),
        ] {
            assert!(
                validate_netrc(&context(&source, password)).is_none(),
                "unexpected cross-record .netrc validation",
            );
        }
    }

    #[test]
    fn ignores_keywords_inside_comments() {
        let password = "CorrectHorseBatteryStaple";
        let source = format!(
            "# machine registry.example.com login alice\n\
             password {password}"
        );

        assert!(validate_netrc(&context(&source, password)).is_none());
    }

    #[test]
    fn rejects_placeholders_and_documentation_values() {
        for password in [
            "changeme",
            "password",
            "your_password",
            "your_password_here",
            "example_password",
            "replace_me",
        ] {
            let source = format!("machine registry.example.com login alice password {password}");

            assert!(
                validate_netrc(&context(&source, password)).is_none(),
                "unexpected documentation password acceptance",
            );
        }
    }
}
