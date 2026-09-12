//! Reusable command-line interface for Cribra.
//!

//! This crate owns command parsing and CLI execution semantics.

//! Detection, validation, findings, remediation, and transformations remain

//! authoritative in the `cribra` core crate.

mod input;

use std::{
    ffi::{OsStr, OsString},
    process::ExitCode,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Executes the Cribra command-line interface.
pub fn run<I, S>(args: I) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();

    match args.next() {
        None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some(argument) if argument == OsStr::new("scan") => {
            let Some(input) = args.next() else {
                eprintln!("cribra: scan requires an input path or '-' for stdin");
                eprintln!("Try 'cribra --help' for usage.");
                return ExitCode::from(2);
            };

            if args.next().is_some() {
                eprintln!("cribra: scan accepts exactly one input");
                eprintln!("Try 'cribra --help' for usage.");
                return ExitCode::from(2);
            }

            let input = input::Input::parse(&input.to_string_lossy());

            match input::read(&input) {
                Ok(source) => {
                    let scanner = cribra::Scanner::default();
                    let _results = scanner.scan([(source.name(), source.text())]);

                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("cribra: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Some(argument) if argument == OsStr::new("--help") || argument == OsStr::new("-h") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some(argument) if argument == OsStr::new("--version") || argument == OsStr::new("-V") => {
            println!("cribra {VERSION}");
            ExitCode::SUCCESS
        }
        Some(argument) => {
            eprintln!("cribra: unknown argument: {}", argument.to_string_lossy());
            eprintln!("Try 'cribra --help' for usage.");
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!(
        "\
Cribra

Usage:
  cribra scan <FILE>
  cribra scan -
  cribra [OPTIONS]

Commands:
  scan <FILE>       Scan one explicit UTF-8 file
  scan -            Scan UTF-8 from standard input

Options:
  -h, --help        Print help
  -V, --version     Print version"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_succeeds() {
        assert_eq!(run(["cribra"]), ExitCode::SUCCESS);
    }

    #[test]
    fn help_succeeds() {
        assert_eq!(run(["cribra", "--help"]), ExitCode::SUCCESS);
        assert_eq!(run(["cribra", "-h"]), ExitCode::SUCCESS);
    }

    #[test]
    fn version_succeeds() {
        assert_eq!(run(["cribra", "--version"]), ExitCode::SUCCESS);
        assert_eq!(run(["cribra", "-V"]), ExitCode::SUCCESS);
    }

    #[test]
    fn unknown_argument_returns_usage_error() {
        assert_eq!(run(["cribra", "--unknown"]), ExitCode::from(2));
    }

    #[test]
    fn scan_requires_one_explicit_input() {
        assert_eq!(run(["cribra", "scan"]), ExitCode::from(2));
    }

    #[test]
    fn scan_rejects_multiple_inputs() {
        assert_eq!(
            run(["cribra", "scan", "one.env", "two.env"]),
            ExitCode::from(2),
        );
    }

    #[test]
    fn scan_accepts_one_utf8_file() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "cribra-cli-scan-{}-{unique}.env",
            std::process::id()
        ));

        fs::write(
            &path,
            b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n",
        )
        .unwrap();

        let exit = run([
            OsString::from("cribra"),
            OsString::from("scan"),
            path.as_os_str().to_owned(),
        ]);

        fs::remove_file(path).unwrap();

        assert_eq!(exit, ExitCode::SUCCESS);
    }
}
