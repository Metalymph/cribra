use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

fn cribra_bin() -> &'static str {
    env!("CARGO_BIN_EXE_cribra")
}

fn temp_file(name: &str, contents: &[u8]) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();

    let path =
        std::env::temp_dir().join(format!("cribra-cli-{name}-{}-{unique}", std::process::id()));

    fs::write(&path, contents).expect("temporary fixture should be written");
    path
}

#[test]
fn file_input_human_output_is_stable() {
    let secret = "ghp_AbCdEf0123456789_AbCdEf0123456789";
    let path = temp_file("finding.env", format!("GITHUB_TOKEN={secret}\n").as_bytes());

    let output = Command::new(cribra_bin())
        .args(["scan", path.to_str().unwrap()])
        .output()
        .expect("cribra should execute");

    fs::remove_file(path).unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.contains("status: findings"));
    assert!(stdout.contains("github.classic-pat"));
    assert!(stdout.contains("severity=critical"));
    assert!(stdout.contains("confidence=high"));
    assert!(!stdout.contains(secret));
    assert!(stderr.is_empty());
}

#[test]
fn file_input_json_output_is_stable() {
    let secret = "ghp_AbCdEf0123456789_AbCdEf0123456789";
    let path = temp_file(
        "finding-json.env",
        format!("GITHUB_TOKEN={secret}\n").as_bytes(),
    );

    let output = Command::new(cribra_bin())
        .args(["scan", path.to_str().unwrap(), "--format", "json"])
        .output()
        .expect("cribra should execute");

    fs::remove_file(path).unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.starts_with('{'));
    assert!(stdout.ends_with("}\n"));
    assert!(stdout.contains("\"status\":\"findings\""));
    assert!(stdout.contains("\"rule_id\":\"github.classic-pat\""));
    assert!(!stdout.contains(secret));
    assert!(stderr.is_empty());
}

#[test]
fn stdin_and_file_have_equivalent_detection_semantics() {
    let source = b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n";
    let path = temp_file("equivalence.env", source);

    let file_output = Command::new(cribra_bin())
        .args(["scan", path.to_str().unwrap(), "--format", "json"])
        .output()
        .expect("file scan should execute");

    let mut child = Command::new(cribra_bin())
        .args(["scan", "-", "--format", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("stdin scan should start");

    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(source)
        .expect("source should be written");

    let stdin_output = child
        .wait_with_output()
        .expect("stdin scan should complete");

    fs::remove_file(path).unwrap();

    assert!(file_output.status.success());
    assert!(stdin_output.status.success());

    let file_stdout = String::from_utf8(file_output.stdout).unwrap();
    let stdin_stdout = String::from_utf8(stdin_output.stdout).unwrap();

    assert!(file_stdout.contains("\"status\":\"findings\""));
    assert!(stdin_stdout.contains("\"status\":\"findings\""));

    assert!(file_stdout.contains("\"rule_id\":\"github.classic-pat\""));
    assert!(stdin_stdout.contains("\"rule_id\":\"github.classic-pat\""));

    assert!(file_stdout.contains("\"findings_count\":1"));
    assert!(stdin_stdout.contains("\"findings_count\":1"));
}

#[test]
fn candidate_only_exit_is_success() {
    let path = temp_file("candidate.txt", b"ABCD-EFGH-IJKL-MNOP\n");

    let output = Command::new(cribra_bin())
        .args(["scan", path.to_str().unwrap()])
        .output()
        .expect("cribra should execute");

    fs::remove_file(path).unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("status: review"));
    assert!(stdout.contains("findings: 0"));
    assert!(stdout.contains("candidates: 1"));
}

#[test]
fn clean_input_exit_is_success() {
    let path = temp_file("clean.txt", b"ordinary text\n");

    let output = Command::new(cribra_bin())
        .args(["scan", path.to_str().unwrap()])
        .output()
        .expect("cribra should execute");

    fs::remove_file(path).unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("status: clean"));
}

#[test]
fn usage_error_returns_exit_code_two() {
    let output = Command::new(cribra_bin())
        .arg("--unknown")
        .output()
        .expect("cribra should execute");

    assert_eq!(output.status.code(), Some(2));

    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stderr.contains("unknown argument"));
    assert!(stderr.contains("Try 'cribra --help' for usage."));
}

#[test]
fn missing_file_returns_exit_code_one() {
    let path = std::env::temp_dir().join(format!("cribra-cli-missing-{}", std::process::id()));

    let _ = fs::remove_file(&path);

    let output = Command::new(cribra_bin())
        .args(["scan", path.to_str().unwrap()])
        .output()
        .expect("cribra should execute");

    assert_eq!(output.status.code(), Some(1));

    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stderr.starts_with("cribra: "));
}

#[test]
fn invalid_utf8_returns_exit_code_one_without_source_leakage() {
    let path = temp_file("invalid-utf8.bin", &[0xf0, 0x28, 0x8c, 0x28]);

    let output = Command::new(cribra_bin())
        .args(["scan", path.to_str().unwrap()])
        .output()
        .expect("cribra should execute");

    fs::remove_file(path).unwrap();

    assert_eq!(output.status.code(), Some(1));

    let stderr = String::from_utf8(output.stderr).unwrap();

    assert_eq!(stderr, "cribra: invalid UTF-8\n");
    assert!(!stderr.contains("\\xf0"));
}
