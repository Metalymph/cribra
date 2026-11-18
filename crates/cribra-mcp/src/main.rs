//! Process entry point for the `cribra-mcp` stdio server.
//!
//! With no arguments the process becomes the MCP server and owns stdout
//! exclusively. `--version` and `--help` are the only alternative behaviors, and
//! neither is reachable while serving, so stdout can never carry a banner,
//! greeting, or diagnostic into the JSON-RPC stream.

use std::io::Write;

const USAGE: &str = "\
cribra-mcp - Cribra Model Context Protocol stdio server

Usage:
  cribra-mcp            Serve MCP over stdin/stdout
  cribra-mcp --version  Print the server version
  cribra-mcp --help     Print this help text

The server speaks newline-delimited JSON-RPC 2.0 on stdin/stdout using the
modern MCP 2026-07-28 revision and the legacy 2025-11-25 lifecycle. While
serving, stdout carries protocol messages only; diagnostics go to stderr.
";

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    match (arguments.next(), arguments.next()) {
        (None, _) => std::process::exit(cribra_mcp::server::run()),
        (Some(flag), None) if flag == "--version" || flag == "-V" => {
            write_stdout_text(&format!("cribra-mcp {}\n", env!("CARGO_PKG_VERSION")));
        }
        (Some(flag), None) if flag == "--help" || flag == "-h" => {
            write_stdout_text(USAGE);
        }
        _ => {
            eprintln!("cribra-mcp: unrecognized argument");
            eprintln!("Try 'cribra-mcp --help' for usage information.");
            std::process::exit(cribra_mcp::limits::EXIT_CONFIG);
        }
    }
}

fn write_stdout_text(text: &str) {
    let mut stdout = std::io::stdout().lock();
    // A closed stdout is not a startup failure for an informational flag.
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}
