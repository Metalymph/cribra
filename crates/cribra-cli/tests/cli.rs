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

    fs::remove_file(&path).unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert_eq!(
        stdout,
        format!(
            "source: {}\nstatus: findings\nfindings: 1\ncandidates: 0\n\
finding: github.classic-pat 1:14 bytes=13..50 severity=critical confidence=high remediation=revoke-and-rotate-credential\n",
            path.display()
        )
    );
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

    fs::remove_file(&path).unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert_eq!(
        stdout,
        format!(
            "{{\"source\":\"{}\",\"status\":\"findings\",\"findings_count\":1,\"candidates_count\":0,\"findings\":[\
{{\"rule_id\":\"github.classic-pat\",\"start\":13,\"end\":50,\"line\":1,\"column\":14,\"severity\":\"critical\",\"confidence\":\"high\",\"remediation\":\"revoke-and-rotate-credential\"}}],\"candidates\":[]}}\n",
            path.display().to_string().replace('\\', "\\\\")
        )
    );
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
    assert!(stdout.contains("kind=recovery-like-code evidence=structural"));
    assert!(!stdout.contains("ABCD-EFGH-IJKL-MNOP"));
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

const QUERY_SOURCE: &str = "\
GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789
password=CorrectHorseBatteryStaple!
password_hash=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
API_KEY=AbCdEfGhIjKlMnOpQrStUvWx
ABCD-EFGH-IJKL-MNOP
";
const QUERY_RULES: [&str; 4] = [
    "github.classic-pat",
    "generic.password-field",
    "generic.sensitive-hash",
    "generic.api-key",
];

fn scan_output(source: &str, options: &[&str], format: &str, stdin: bool) -> String {
    let mut command = Command::new(cribra_bin());
    command.arg("scan");
    let path = if stdin {
        command.arg("-").stdin(Stdio::piped());
        None
    } else {
        let path = temp_file("query.env", source.as_bytes());
        command.arg(&path);
        Some(path)
    };
    command
        .args(["--format", format])
        .args(options)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("scan should start");
    if stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    if let Some(path) = path {
        fs::remove_file(path).unwrap();
    }
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Check every fixture value, including candidate material, in both formats.
    for line in source.lines() {
        let value = line.split_once('=').map_or(line, |(_, value)| value);
        if !value.is_empty() {
            assert!(!stdout.contains(value), "source value must not be rendered");
        }
    }
    stdout
}

fn rendered_rule_ids<'a>(output: &'a str, format: &str) -> Vec<&'a str> {
    if format == "json" {
        output
            .split("\"rule_id\":\"")
            .skip(1)
            .map(|tail| tail.split('"').next().unwrap())
            .collect()
    } else {
        output
            .lines()
            .filter_map(|line| line.strip_prefix("finding: "))
            .map(|tail| tail.split(' ').next().unwrap())
            .collect()
    }
}

fn assert_selection(output: &str, format: &str, expected: &[&str]) {
    let rendered = rendered_rule_ids(output, format);
    assert_eq!(rendered, expected);
    if format == "json" {
        assert!(
            output.contains("\"status\":\"findings\",\"findings_count\":4,\"candidates_count\":1,")
        );
        assert!(output.contains(&format!(
            "\"selection\":{{\"active\":true,\"findings_count\":{}}}",
            rendered.len()
        )));
        assert!(output.ends_with(
            "\"candidates\":[{\"start\":199,\"end\":218,\"line\":5,\"column\":1,\"kind\":\"recovery-like-code\",\"evidence\":\"structural\"}]}\n"
        ));
    } else {
        assert!(output.contains(&format!(
            "status: findings\nfindings: 4\ncandidates: 1\nselected findings: {}\n",
            rendered.len()
        )));
        assert!(output.ends_with(
            "candidate: 5:1 bytes=199..218 kind=recovery-like-code evidence=structural\n"
        ));
    }
}

#[test]
fn finding_filters_select_metadata_in_core_order() {
    let cases: &[(&[&str], &[&str])] = &[
        (&["--severity", "high"], &[QUERY_RULES[1], QUERY_RULES[3]]),
        (
            &["--min-severity", "high"],
            &[QUERY_RULES[0], QUERY_RULES[1], QUERY_RULES[3]],
        ),
        (&["--confidence", "medium"], &QUERY_RULES[1..]),
        (&["--min-confidence", "medium"], &QUERY_RULES),
        (&["--confidence", "high"], &QUERY_RULES[..1]),
        (&["--min-confidence", "high"], &QUERY_RULES[..1]),
        (&["--rule-id", "generic.api-key"], &QUERY_RULES[3..]),
        (
            &["--min-severity", "high", "--confidence", "medium"],
            &[QUERY_RULES[1], QUERY_RULES[3]],
        ),
        (
            &[
                "--severity",
                "high",
                "--min-confidence",
                "medium",
                "--rule-id",
                "generic.api-key",
            ],
            &QUERY_RULES[3..],
        ),
    ];
    for format in ["human", "json"] {
        for &(options, expected) in cases {
            let output = scan_output(QUERY_SOURCE, options, format, false);
            assert_selection(&output, format, expected);
        }
    }
}

#[test]
fn finding_sorts_match_core_in_both_directions_without_filters() {
    let scanner = cribra::Scanner::default();
    let results = scanner.scan([("fixture", QUERY_SOURCE)]);
    for (option, sort) in [
        ("rule-id", cribra::ScanSort::RuleId),
        ("rule-id-desc", cribra::ScanSort::RuleIdDescending),
        ("severity", cribra::ScanSort::Severity),
        ("severity-desc", cribra::ScanSort::SeverityDescending),
        ("confidence", cribra::ScanSort::Confidence),
        ("confidence-desc", cribra::ScanSort::ConfidenceDescending),
        ("location", cribra::ScanSort::Location),
        ("location-desc", cribra::ScanSort::LocationDescending),
    ] {
        let sorted = results.query().sort(sort);
        let expected: Vec<_> = sorted
            .iter()
            .map(|(_, finding)| finding.rule_id().as_str())
            .collect();
        for format in ["human", "json"] {
            let output = scan_output(QUERY_SOURCE, &["--sort", option], format, false);
            assert_selection(&output, format, &expected);
        }
    }
}

#[test]
fn combined_filter_and_sort_agree_for_file_and_stdin() {
    for format in ["human", "json"] {
        for stdin in [false, true] {
            let output = scan_output(
                QUERY_SOURCE,
                &[
                    "--min-severity",
                    "high",
                    "--confidence",
                    "medium",
                    "--sort",
                    "rule-id",
                ],
                format,
                stdin,
            );
            assert_selection(&output, format, &[QUERY_RULES[3], QUERY_RULES[1]]);
        }
    }
}

#[test]
fn empty_selection_preserves_original_findings_and_candidates() {
    for options in [
        &["--severity", "info"][..],
        &["--confidence", "low"][..],
        &["--rule-id", "unknown.rule"][..],
        &["--severity", "medium", "--rule-id", "github.classic-pat"][..],
    ] {
        for format in ["human", "json"] {
            let output = scan_output(QUERY_SOURCE, options, format, true);
            assert_selection(&output, format, &[]);
        }
    }
}

#[test]
fn candidate_only_and_clean_selections_preserve_original_state() {
    for (source, status, candidates) in [("ABCD-EFGH-IJKL-MNOP\n", "review", 1), ("", "clean", 0)] {
        for format in ["human", "json"] {
            let baseline = scan_output(source, &[], format, true);
            if format == "json" {
                let candidate_json = if candidates == 1 {
                    "{\"start\":0,\"end\":19,\"line\":1,\"column\":1,\"kind\":\"recovery-like-code\",\"evidence\":\"structural\"}"
                } else {
                    ""
                };
                assert_eq!(
                    baseline,
                    format!(
                        "{{\"source\":\"<stdin>\",\"status\":\"{status}\",\"findings_count\":0,\"candidates_count\":{candidates},\"findings\":[],\"candidates\":[{candidate_json}]}}\n"
                    )
                );
            } else {
                let candidate_line = if candidates == 1 {
                    "candidate: 1:1 bytes=0..19 kind=recovery-like-code evidence=structural\n"
                } else {
                    ""
                };
                assert_eq!(
                    baseline,
                    format!(
                        "source: <stdin>\nstatus: {status}\nfindings: 0\ncandidates: {candidates}\n{candidate_line}"
                    )
                );
            }
            for options in [
                &["--rule-id", "github.classic-pat"][..],
                &["--sort", "severity-desc"][..],
            ] {
                let selected = scan_output(source, options, format, true);
                assert!(rendered_rule_ids(&selected, format).is_empty());
                if format == "json" {
                    assert!(selected.contains(&format!(
                        "\"status\":\"{status}\",\"findings_count\":0,\"candidates_count\":{candidates},"
                    )));
                    assert_eq!(
                        selected
                            .replace("\"selection\":{\"active\":true,\"findings_count\":0},", ""),
                        baseline
                    );
                } else {
                    assert!(selected.contains(&format!(
                        "status: {status}\nfindings: 0\ncandidates: {candidates}\n"
                    )));
                    assert_eq!(selected.replace("selected findings: 0\n", ""), baseline);
                }
            }
        }
    }
}

fn without_detection(output: &str, format: &str) -> String {
    let mut baseline = output.to_owned();
    for mode in ["matcher-only", "deterministic", "contextual"] {
        let field = if format == "json" {
            format!(",\"detection_mode\":\"{mode}\"")
        } else {
            format!(" detection={mode}")
        };
        baseline = baseline.replace(&field, "");
    }
    baseline
}

#[test]
fn explain_projects_all_default_detection_modes_without_activating_selection() {
    let source = format!(
        "{QUERY_SOURCE}-----BEGIN PRIVATE KEY-----\nSyntheticPrivateKeyMaterial123456\n-----END PRIVATE KEY-----\n"
    );
    for format in ["human", "json"] {
        let baseline = scan_output(&source, &[], format, true);
        let explained = scan_output(&source, &["--explain"], format, true);
        let modes: Vec<_> = if format == "json" {
            assert!(!explained.contains("\"selection\":"));
            explained
                .split("\"detection_mode\":\"")
                .skip(1)
                .map(|tail| tail.split('"').next().unwrap())
                .collect()
        } else {
            assert!(!explained.contains("selected findings:"));
            explained
                .lines()
                .filter_map(|line| line.split_once(" detection=").map(|(_, mode)| mode))
                .collect()
        };
        assert_eq!(
            modes,
            [
                "deterministic",
                "contextual",
                "contextual",
                "contextual",
                "matcher-only"
            ]
        );
        assert_eq!(without_detection(&explained, format), baseline);
    }
}

#[test]
fn explain_composes_with_every_filter_and_sort() {
    let options: &[&[&str]] = &[
        &["--severity", "high"],
        &["--min-severity", "high"],
        &["--confidence", "medium"],
        &["--min-confidence", "medium"],
        &["--rule-id", "generic.api-key"],
        &["--sort", "rule-id"],
        &["--sort", "rule-id-desc"],
        &["--sort", "severity"],
        &["--sort", "severity-desc"],
        &["--sort", "confidence"],
        &["--sort", "confidence-desc"],
        &["--sort", "location"],
        &["--sort", "location-desc"],
        &[
            "--severity",
            "high",
            "--min-confidence",
            "medium",
            "--rule-id",
            "generic.api-key",
            "--sort",
            "location-desc",
        ],
        &["--rule-id", "missing.rule"],
    ];
    for format in ["human", "json"] {
        for &options in options {
            let baseline = scan_output(QUERY_SOURCE, options, format, true);
            let with_explain: Vec<_> = options.iter().copied().chain(["--explain"]).collect();
            let explained = scan_output(QUERY_SOURCE, &with_explain, format, true);
            assert_eq!(without_detection(&explained, format), baseline);
            let detection_field = if format == "json" {
                "\"detection_mode\":"
            } else {
                " detection="
            };
            assert_eq!(
                explained.matches(detection_field).count(),
                rendered_rule_ids(&explained, format).len()
            );
        }
    }
}

#[test]
fn explain_with_selection_preserves_file_and_stdin_semantics() {
    for format in ["human", "json"] {
        for stdin in [false, true] {
            let output = scan_output(
                QUERY_SOURCE,
                &[
                    "--explain",
                    "--min-severity",
                    "high",
                    "--confidence",
                    "medium",
                    "--sort",
                    "rule-id",
                ],
                format,
                stdin,
            );
            assert_selection(&output, format, &[QUERY_RULES[3], QUERY_RULES[1]]);
            let detection_field = if format == "json" {
                "\"detection_mode\":\"contextual\""
            } else {
                " detection=contextual"
            };
            assert_eq!(output.matches(detection_field).count(), 2);
            assert!(!output.contains("deterministic"));
        }
    }
}

#[test]
fn explain_preserves_candidate_only_and_clean_output() {
    for source in ["ABCD-EFGH-IJKL-MNOP\n", ""] {
        for format in ["human", "json"] {
            let baseline = scan_output(source, &[], format, true);
            assert_eq!(scan_output(source, &["--explain"], format, true), baseline);
        }
    }
}

#[test]
fn explain_preserves_input_and_usage_error_exit_codes() {
    let path = temp_file("explain-invalid.bin", &[0xff]);
    let output = Command::new(cribra_bin())
        .arg("scan")
        .arg(&path)
        .arg("--explain")
        .output()
        .unwrap();
    fs::remove_file(&path).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"cribra: invalid UTF-8\n");

    let missing = Command::new(cribra_bin())
        .arg("scan")
        .arg(&path)
        .arg("--explain")
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());

    let duplicate = Command::new(cribra_bin())
        .args(["scan", "-", "--explain", "--explain"])
        .output()
        .unwrap();
    assert_eq!(duplicate.status.code(), Some(2));
    assert!(duplicate.stdout.is_empty());
}
