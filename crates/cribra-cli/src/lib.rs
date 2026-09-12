//! Reusable command-line interface for Cribra.
//!

//! This crate owns command parsing and CLI execution semantics.

//! Detection, validation, findings, remediation, and transformations remain

//! authoritative in the `cribra` core crate.

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
        "

Cribra

Usage:

  cribra [OPTIONS]

Options:

  -h, --help       Print help

  -V, --version    Print version"
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
}
