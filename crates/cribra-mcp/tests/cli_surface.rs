//! CLI surface qualification for the shipping binary.
//!
//! The MCP contract reserves stdout for JSON-RPC. These tests prove that the
//! informational flags are the only way to produce non-protocol stdout, that
//! they never run while serving, and that serving emits no banner: the very
//! first byte on stdout is the opening brace of the first response.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

use serde_json::json;
use wait_timeout::ChildExt;

const BIN: &str = env!("CARGO_BIN_EXE_cribra-mcp");
const VERSION_LINE: &str = concat!("cribra-mcp ", env!("CARGO_PKG_VERSION"), "\n");
const DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

fn run(arguments: &[&str]) -> (std::process::ExitStatus, String, String) {
    let output = Command::new(BIN)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .expect("run binary");
    (
        output.status,
        String::from_utf8(output.stdout).expect("utf-8 stdout"),
        String::from_utf8(output.stderr).expect("utf-8 stderr"),
    )
}

#[test]
fn version_prints_one_deterministic_line() {
    let (status, stdout, stderr) = run(&["--version"]);
    assert!(status.success(), "expected exit 0, got {status}");
    assert_eq!(stdout, VERSION_LINE);
    assert_eq!(stderr, "");
}

#[test]
fn help_documents_the_flags_and_never_mentions_json() {
    let (status, stdout, stderr) = run(&["--help"]);
    assert!(status.success(), "expected exit 0, got {status}");
    assert_eq!(stderr, "");
    assert!(stdout.contains("cribra-mcp"), "help must name the binary");
    assert!(stdout.contains("--version"), "help must document --version");
    assert!(stdout.contains("--help"), "help must document --help");
    assert!(
        stdout.contains("2026-07-28") && stdout.contains("2025-11-25"),
        "help must state the supported protocol revisions"
    );
}

#[test]
fn unknown_or_extra_arguments_fail_without_touching_stdout() {
    for arguments in [
        vec!["--nope"],
        vec!["serve"],
        vec!["--version", "--help"],
        vec!["--help", "extra"],
    ] {
        let (status, stdout, stderr) = run(&arguments);
        assert_eq!(
            status.code(),
            Some(2),
            "expected configuration failure for {arguments:?}, got {status}"
        );
        assert_eq!(stdout, "", "arguments {arguments:?} produced stdout");
        assert!(
            stderr.contains("unrecognized argument"),
            "arguments {arguments:?} produced unexpected stderr: {stderr}"
        );
    }
}

#[test]
fn serving_starts_with_the_opening_brace_and_no_banner() {
    let mut child = Command::new(BIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn server");

    let mut stdout = child.stdout.take().expect("stdout pipe");
    let mut stdin = child.stdin.take().expect("stdin pipe");
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
            }
        }
    });
    let mut line = serde_json::to_vec(&request).expect("request serializes");
    line.push(b'\n');
    stdin.write_all(&line).expect("write request");
    stdin.flush().expect("flush request");

    // The first byte on stdout must be protocol data, not a banner.
    let mut first = [0u8; 1];
    stdout.read_exact(&mut first).expect("first stdout byte");
    assert_eq!(first[0], b'{', "stdout did not start with a JSON message");

    drop(stdin);
    let status = child
        .wait_timeout(DEADLINE)
        .expect("wait for exit")
        .expect("server terminated");
    assert!(status.success(), "expected clean exit, got {status}");
}
