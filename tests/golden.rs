//! Golden tests for the canonical fixture corpus.
//!
//! These tests NEVER update committed outputs. Regeneration is an explicit
//! maintainer action performed by `examples/generate_fixtures.rs`.

#![cfg(feature = "serde")]

use std::{
    fs,
    path::{Path, PathBuf},
};

use silens_scan::{
    Scanner,
    transform::{
        PseudonymizationOptions, SynthesisOptions, pseudonymize, redact, synthesize, template,
    },
};

#[path = "../examples/fixtures/corpus.rs"]
mod corpus;

#[test]
fn canonical_outputs_match_committed_golden_files() {
    let fixture_root = fixture_root();
    let inputs = fixture_root.join("inputs");
    let outputs = fixture_root.join("outputs");
    let scanner = corpus::scanner().expect("canonical scanner should build");

    let input_files = sorted_input_files(&inputs).expect("fixture inputs should be readable");

    assert!(
        !input_files.is_empty(),
        "canonical fixture corpus must contain inputs",
    );

    for input_path in input_files {
        verify_fixture(&scanner, &inputs, &outputs, &input_path);
    }
}

#[test]
fn output_tree_contains_one_artifact_per_input_per_category() {
    let fixture_root = fixture_root();
    let inputs = fixture_root.join("inputs");
    let outputs = fixture_root.join("outputs");

    let input_files = sorted_input_files(&inputs).unwrap();
    let input_count = input_files.len();

    assert!(input_count > 0);

    for category in [
        "reports",
        "redacted",
        "templates",
        "pseudonymized",
        "synthesized",
    ] {
        let count = recursive_file_count(&outputs.join(category)).unwrap_or(0);

        assert_eq!(
            count, input_count,
            "{category} must contain exactly one golden artifact per input",
        );
    }
}

#[test]
fn clean_and_false_positive_fixtures_remain_unchanged_by_transformations() {
    let fixture_root = fixture_root();

    for name in ["clean.env", "false-positives.txt"] {
        let input = fs::read(fixture_root.join("inputs").join(name)).unwrap();

        for category in ["redacted", "templates", "pseudonymized", "synthesized"] {
            let output = fs::read(fixture_root.join("outputs").join(category).join(name)).unwrap();

            assert_eq!(
                output, input,
                "{category}/{name} must remain byte-identical when no findings exist",
            );
        }
    }
}

#[test]
fn transformed_golden_outputs_do_not_retain_canonical_sensitive_values() {
    let fixture_root = fixture_root();

    let forbidden = [
        "DEMO_SECRET_ALPHA",
        "DEMO_PRIVATE_KEY_MATERIAL",
        "demo_api_",
        "demo-pass-",
    ];

    for category in ["redacted", "templates", "pseudonymized", "synthesized"] {
        for path in recursive_files(&fixture_root.join("outputs").join(category)).unwrap() {
            let content = fs::read_to_string(&path).unwrap();

            for token in forbidden {
                assert!(
                    !content.contains(token),
                    "{} unexpectedly retains canonical sensitive marker {token}",
                    path.display(),
                );
            }
        }
    }
}

#[test]
fn report_golden_outputs_never_contain_source_values() {
    let fixture_root = fixture_root();

    let forbidden = [
        "DEMO_SECRET_ALPHA",
        "DEMO_PRIVATE_KEY_MATERIAL",
        "demo_api_",
        "demo-pass-",
    ];

    for path in recursive_files(&fixture_root.join("outputs/reports")).unwrap() {
        let content = fs::read_to_string(&path).unwrap();

        for token in forbidden {
            assert!(
                !content.contains(token),
                "{} leaks source material into structured report output",
                path.display(),
            );
        }
    }
}

#[test]
fn crlf_fixture_preserves_crlf_outside_replaced_spans() {
    let fixture_root = fixture_root();

    for category in ["redacted", "templates", "pseudonymized", "synthesized"] {
        let bytes = fs::read(fixture_root.join("outputs").join(category).join("crlf.env")).unwrap();

        assert!(
            bytes.windows(2).any(|window| window == b"\r\n"),
            "{category}/crlf.env must preserve CRLF line endings",
        );
        assert!(
            !bytes
                .windows(1)
                .enumerate()
                .any(|(index, window)| window == b"\n" && (index == 0 || bytes[index - 1] != b'\r')),
            "{category}/crlf.env introduced bare LF line endings",
        );
    }
}

fn verify_fixture(scanner: &Scanner, inputs: &Path, outputs: &Path, input_path: &Path) {
    let source = fs::read_to_string(input_path).unwrap();
    let relative = input_path
        .strip_prefix(inputs)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");

    let results = scanner.scan([(relative.as_str(), source.as_str())]);
    let report = results.single_report().expect("one source scanned");

    let actual_report = serde_json::to_string_pretty(&results).unwrap() + "\n";
    assert_golden(
        outputs.join("reports").join(format!("{relative}.json")),
        actual_report.as_bytes(),
    );

    assert_golden(
        outputs.join("redacted").join(&relative),
        redact(&source, report).unwrap().as_bytes(),
    );

    assert_golden(
        outputs.join("templates").join(&relative),
        template(&source, report).unwrap().as_bytes(),
    );

    assert_golden(
        outputs.join("pseudonymized").join(&relative),
        pseudonymize(
            &source,
            report,
            &PseudonymizationOptions::new(corpus::PSEUDONYMIZATION_KEY),
        )
        .unwrap()
        .as_bytes(),
    );

    assert_golden(
        outputs.join("synthesized").join(&relative),
        synthesize(
            &source,
            report,
            &SynthesisOptions::new(corpus::SYNTHESIS_KEY),
        )
        .unwrap()
        .as_bytes(),
    );
}

fn assert_golden(path: PathBuf, actual: &[u8]) {
    let expected = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "missing/unreadable golden file {}: {error}. Regenerate explicitly with \
             `cargo run --example generate_fixtures --features serde`",
            path.display(),
        )
    });

    assert_eq!(
        actual,
        expected,
        "golden mismatch for {}. Review the behaviour change before regenerating fixtures",
        path.display(),
    );
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/fixtures")
}

fn sorted_input_files(inputs: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = recursive_files(inputs)?;
    files.sort();
    Ok(files)
}

fn recursive_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if !root.exists() {
        return Ok(files);
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        if entry.file_type()?.is_dir() {
            files.extend(recursive_files(&path)?);
        } else if entry.file_type()?.is_file() {
            files.push(path);
        }
    }

    Ok(files)
}

fn recursive_file_count(root: &Path) -> std::io::Result<usize> {
    Ok(recursive_files(root)?.len())
}
