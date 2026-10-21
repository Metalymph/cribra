#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use cribra::{
    Scanner,
    transform::{PseudonymizationOptions, ShareBundle, ShareMode, SynthesisOptions},
};
use cribra_cli::{BundleCommand, BundleMode, Command as CliCommand, ExecuteError, execute};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        for _ in 0..128 {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "cribra-export-{}-{nonce:x}-{sequence:x}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Self { root },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("temporary fixture directory should be created: {error}"),
            }
        }
        panic!("temporary fixture directory should be available");
    }
    fn write(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, contents).unwrap();
        path
    }
    fn run(&self, mode: &str, output: &Path, sources: &[PathBuf], key: Option<&Path>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cribra"));
        command
            .args(["bundle", "--mode", mode, "--output"])
            .arg(output);
        if let Some(key) = key {
            command.arg("--key-file").arg(key);
        }
        command.args(sources).output().unwrap()
    }
    fn no_staging(&self) {
        assert!(fs::read_dir(&self.root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .as_encoded_bytes()
                .starts_with(b".cribra-bundle-")
        }));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn failed(output: &Output) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn all_modes_export_exact_manifest_contents_order_and_private_permissions() {
    let fixture = Fixture::new();
    let key = *b"0123456789ABCDEFfedcba9876543210";
    let key_file = fixture.write("private-key.bin", &key);
    let originals = [
        "\u{feff}π 😀\r\nGITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\r\npassword=CorrectHorseBatteryStaple!\r\npassword_hash=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\r\nABCD-EFGH-IJKL-MNOP",
        "ABCD-EFGH-IJKL-MNOP\r\n",
        "",
        "\u{feff}π 😀 café\r\nordinary text\r\n",
    ];
    let paths: Vec<_> = ["z.env", "a.env", "m.env", "b.env"]
        .into_iter()
        .zip(originals)
        .map(|(name, text)| fixture.write(name, text.as_bytes()))
        .collect();
    let scanned_bytes: usize = originals.iter().map(|source| source.len()).sum();
    let scanner = Scanner::default();
    for (name, mode, key_path) in [
        ("redact", ShareMode::Redact, None),
        ("template", ShareMode::Template, None),
        (
            "pseudonymize",
            ShareMode::Pseudonymize(PseudonymizationOptions::new(key)),
            Some(key_file.as_path()),
        ),
        (
            "synthesize",
            ShareMode::Synthesize(SynthesisOptions::new(key)),
            Some(key_file.as_path()),
        ),
    ] {
        let output_dir = fixture.root.join(name);
        let output = fixture.run(name, &output_dir, &paths, key_path);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        let core = ShareBundle::builder()
            .mode(mode)
            .scan_and_build(
                &scanner,
                originals.iter().enumerate().map(|(i, source)| (i, *source)),
            )
            .unwrap();
        let expected_manifest = format!(
            concat!(
                "{{\"schema_version\":1,\"mode\":\"{}\",\"sources\":[\"sources/0001.txt\",\"sources/0002.txt\",\"sources/0003.txt\",\"sources/0004.txt\"],",
                "\"summary\":{{\"scanned_sources\":4,\"scanned_bytes\":{},\"reports_with_findings\":1,\"reports_without_findings\":3,",
                "\"total_findings\":3,\"total_candidates\":2,\"reports_with_candidates\":2,\"critical\":1,\"high\":1,\"medium\":1,\"low\":0,\"info\":0}}}}\n"
            ),
            name, scanned_bytes
        );
        assert_eq!(
            fs::read(output_dir.join("manifest.json")).unwrap(),
            expected_manifest.as_bytes()
        );
        assert_eq!(fs::read_dir(&output_dir).unwrap().count(), 2);
        assert_eq!(fs::read_dir(output_dir.join("sources")).unwrap().count(), 4);
        for directory in [&output_dir, &output_dir.join("sources")] {
            assert_eq!(
                fs::metadata(directory).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        for (index, source) in core.sources().iter().enumerate() {
            let path = output_dir.join(format!("sources/{:04}.txt", index + 1));
            assert!(fs::read(&path).unwrap() == source.content().as_bytes());
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(
            fs::metadata(output_dir.join("manifest.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        for (path, original) in paths.iter().zip(originals) {
            assert!(fs::read(path).unwrap() == original.as_bytes());
        }
        assert!(fs::read(&key_file).unwrap() == key);
        fixture.no_staging();
    }
}

#[test]
fn existing_files_directories_and_symlinks_are_never_replaced() {
    let fixture = Fixture::new();
    let source = fixture.write(
        "source.env",
        b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789",
    );
    let directory = fixture.root.join("directory");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("preserve"), b"directory data").unwrap();
    let file = fixture.write("file", b"file data");
    let link = fixture.root.join("link");
    symlink(&directory, &link).unwrap();
    let dangling = fixture.root.join("dangling");
    symlink(fixture.root.join("absent"), &dangling).unwrap();
    for destination in [&directory, &file, &link, &dangling] {
        let output = fixture.run("redact", destination, std::slice::from_ref(&source), None);
        failed(&output);
        fixture.no_staging();
    }
    assert_eq!(
        fs::read(directory.join("preserve")).unwrap(),
        b"directory data"
    );
    assert_eq!(fs::read(&file).unwrap(), b"file data");
    assert_eq!(fs::read_link(link).unwrap(), directory);
    assert_eq!(
        fs::read_link(dangling).unwrap(),
        fixture.root.join("absent")
    );
}

#[test]
fn concurrent_public_exports_publish_exactly_once_without_overwrite() {
    let fixture = Fixture::new();
    let source = fixture.write(
        "source.env",
        b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789",
    );
    let command = BundleCommand {
        mode: BundleMode::Redact,
        output_dir: fixture.root.join("export"),
        sources: vec![source.clone()],
        key_file: None,
    };
    let barrier = Arc::new(Barrier::new(3));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let barrier = barrier.clone();
            let command = command.clone();
            std::thread::spawn(move || {
                barrier.wait();
                execute(&CliCommand::Bundle(command))
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(ExecuteError::BundleExport(_))))
            .count(),
        1
    );
    assert!(
        results
            .iter()
            .filter_map(|result| result.as_ref().ok())
            .all(|output| output.stdout().is_empty())
    );
    assert_eq!(
        fs::read(command.output_dir.join("sources/0001.txt")).unwrap(),
        b"GITHUB_TOKEN=[REDACTED]"
    );
    assert_eq!(
        fs::read(source).unwrap(),
        b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789"
    );
    fixture.no_staging();
}

#[test]
fn preparation_failures_never_publish_or_leave_staging() {
    let fixture = Fixture::new();
    let output_dir = fixture.root.join("export");
    let source = fixture.write("secret-bearing-filename.env", &[0xff]);
    let key = fixture.write("private-key.bin", &[5; 32]);
    let short = fixture.write("short-key.bin", &[5; 31]);
    let long = fixture.write("long-key.bin", &[5; 33]);
    for mode in ["pseudonymize", "synthesize"] {
        for invalid in [
            &short,
            &long,
            &fixture.root.join("missing-key.bin"),
            &fixture.root,
        ] {
            let output = fixture.run(
                mode,
                &output_dir,
                std::slice::from_ref(&source),
                Some(invalid),
            );
            failed(&output);
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(stderr.contains("key file"));
            assert!(!stderr.contains("secret-bearing-filename"));
            assert!(!output_dir.exists());
            fixture.no_staging();
        }
        let output = fixture.run(mode, &output_dir, std::slice::from_ref(&source), Some(&key));
        failed(&output);
        assert_eq!(output.stderr, b"cribra: invalid UTF-8\n");
    }
    let overlap = fixture.write("source-secret-name.txt", b"-----BEGIN PRIVATE KEY-----\nghp_AbCdEf0123456789_AbCdEf0123456789\n-----END PRIVATE KEY-----");
    for mode in ["template", "pseudonymize", "synthesize"] {
        let output = fixture.run(
            mode,
            &output_dir,
            std::slice::from_ref(&overlap),
            (mode != "template").then_some(key.as_path()),
        );
        failed(&output);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("overlap"));
        assert!(!stderr.contains("ghp_"));
        assert!(!stderr.contains("source-secret-name"));
        assert!(!output_dir.exists());
        fixture.no_staging();
    }
}

#[test]
fn staging_and_publication_failures_are_safe_and_cleaned() {
    let fixture = Fixture::new();
    let source = fixture.write("do-not-print-source-name.txt", b"ordinary text");
    let missing_parent = fixture.root.join("missing").join("export");
    let output = fixture.run(
        "redact",
        &missing_parent,
        std::slice::from_ref(&source),
        None,
    );
    failed(&output);
    assert!(!fixture.root.join("missing").exists());
    // The excessively long final component fails only after staging is written.
    let oversized = fixture.root.join("SENSITIVE_DESTINATION_".repeat(200));
    let output = fixture.run("redact", &oversized, std::slice::from_ref(&source), None);
    failed(&output);
    assert!(
        !String::from_utf8(output.stderr)
            .unwrap()
            .contains("SENSITIVE_DESTINATION")
    );
    fixture.no_staging();
    let parent = fixture.root.join("readonly");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
    let probe = parent.join("probe");
    if fs::write(&probe, b"").is_err() {
        let output = fixture.run(
            "redact",
            &parent.join("export"),
            std::slice::from_ref(&source),
            None,
        );
        failed(&output);
        assert!(!parent.join("export").exists());
    } else {
        fs::remove_file(probe).unwrap();
    }
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(fs::read_dir(parent).unwrap().count(), 0);
    fixture.no_staging();
}

#[test]
fn public_bundle_model_is_validated_before_key_or_source_access() {
    let base = BundleCommand {
        mode: BundleMode::Redact,
        output_dir: "missing/export".into(),
        sources: vec!["missing/source".into()],
        key_file: None,
    };
    let mut invalid = Vec::new();
    let mut command = base.clone();
    command.sources.clear();
    invalid.push(command);
    for path in ["", "-", "--source"] {
        let mut command = base.clone();
        command.sources = vec![path.into()];
        invalid.push(command);
        let mut command = base.clone();
        command.output_dir = path.into();
        invalid.push(command);
    }
    let mut command = base.clone();
    command.sources.push(command.sources[0].clone());
    invalid.push(command);
    let mut command = base.clone();
    command.key_file = Some("missing/key".into());
    invalid.push(command);
    for mode in [BundleMode::Pseudonymize, BundleMode::Synthesize] {
        let mut command = base.clone();
        command.mode = mode;
        invalid.push(command);
        for key in ["", "-", "--key"] {
            let mut command = base.clone();
            command.mode = mode;
            command.key_file = Some(key.into());
            invalid.push(command);
        }
    }
    for command in invalid {
        assert!(matches!(
            execute(&CliCommand::Bundle(command)),
            Err(ExecuteError::InvalidBundleCommand(_))
        ));
    }
    let fixture = Fixture::new();
    let source = fixture.write("source.txt", b"ordinary text");
    let malformed = BundleCommand {
        sources: vec![source],
        output_dir: "malformed\0/output".into(),
        ..base
    };
    assert!(matches!(
        execute(&CliCommand::Bundle(malformed)),
        Err(ExecuteError::BundleExport(_))
    ));
    fixture.no_staging();
}

#[test]
fn explicit_filesystem_paths_with_leading_dash_names_are_supported() {
    let fixture = Fixture::new();
    fixture.write("-source", b"ordinary text\r\n");
    fixture.write("-key", &[8; 32]);
    let output = Command::new(env!("CARGO_BIN_EXE_cribra"))
        .current_dir(&fixture.root)
        .args([
            "bundle",
            "--mode",
            "pseudonymize",
            "--key-file",
            "./-key",
            "--output",
            "./-export",
            "./-source",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read(fixture.root.join("-export/sources/0001.txt")).unwrap(),
        b"ordinary text\r\n"
    );
    fixture.no_staging();
}
