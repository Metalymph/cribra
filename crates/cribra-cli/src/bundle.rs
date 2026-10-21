//! Internal, in-memory bundle preparation. This module does not publish files.
//!
//! Transformed contents may retain candidates and undetected sensitive data.
//! They are deliberately not included in Debug or manifest output.

use std::{fmt::Write, io};

use cribra::{
    Scanner,
    transform::{PseudonymizationOptions, ShareBundle, ShareManifest, ShareMode, SynthesisOptions},
};

use crate::{
    BundleCommand, BundleMode, ExecuteError, execute::read_key_file, input,
    output::push_json_string,
};

/// Prepared contents and CLI manifest v1, with no filesystem output side effects.
pub(crate) struct PreparedBundle {
    /// Transformed sources in the original argument order.
    pub(crate) sources: Vec<PreparedSource>,
    /// Deterministic metadata-only JSON, ending in one newline.
    pub(crate) manifest_json: String,
}

/// One owned transformed source; the path is relative and independent of input names.
pub(crate) struct PreparedSource {
    pub(crate) output_path: String,
    pub(crate) content: String,
}

/// Reads explicit sources and delegates the complete batch to Core.
///
/// Keyed modes validate the key before acquiring any source. The requested
/// output directory is never accessed, and no original paths enter the manifest.
pub(crate) fn prepare(command: &BundleCommand) -> Result<PreparedBundle, ExecuteError> {
    let mode = match command.mode {
        BundleMode::Redact => ShareMode::Redact,
        BundleMode::Template => ShareMode::Template,
        BundleMode::Pseudonymize | BundleMode::Synthesize => {
            let key_path = command.key_file.as_deref().ok_or_else(|| {
                ExecuteError::KeyRead(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "keyed bundle preparation requires a key file",
                ))
            })?;
            let key = read_key_file(key_path)?;
            match command.mode {
                BundleMode::Pseudonymize => {
                    ShareMode::Pseudonymize(PseudonymizationOptions::new(key))
                }
                BundleMode::Synthesize => ShareMode::Synthesize(SynthesisOptions::new(key)),
                _ => unreachable!("only keyed modes enter this branch"),
            }
        }
    };

    let sources = command
        .sources
        .iter()
        .map(|path| input::read(&input::Input::File(path.clone())).map_err(ExecuteError::from))
        .collect::<Result<Vec<_>, _>>()?;
    let scanner = Scanner::default();
    let bundle = ShareBundle::builder().mode(mode).scan_and_build(
        &scanner,
        sources
            .iter()
            .enumerate()
            .map(|(index, source)| (index, source.text())),
    )?;
    let (transformed, manifest) = bundle.into_parts();
    let sources: Vec<_> = transformed
        .into_iter()
        .enumerate()
        .map(|(index, source)| PreparedSource {
            output_path: source_output_path(index + 1),
            content: source.into_content(),
        })
        .collect();
    let manifest_json = render_manifest(&manifest, &sources);
    Ok(PreparedBundle {
        sources,
        manifest_json,
    })
}

/// Formats a one-based index with a minimum width, never truncating larger indices.
fn source_output_path(number: usize) -> String {
    format!("sources/{number:04}.txt")
}

fn render_manifest(manifest: &ShareManifest, sources: &[PreparedSource]) -> String {
    let mut output = String::from("{\"schema_version\":1,\"mode\":");
    push_json_string(&mut output, &manifest.mode().to_string());
    output.push_str(",\"sources\":[");
    for (index, source) in sources.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_json_string(&mut output, &source.output_path);
    }
    let summary = manifest.summary();
    write!(
        output,
        concat!(
            "],\"summary\":{{",
            "\"scanned_sources\":{},\"scanned_bytes\":{},",
            "\"reports_with_findings\":{},\"reports_without_findings\":{},",
            "\"total_findings\":{},\"total_candidates\":{},\"reports_with_candidates\":{},",
            "\"critical\":{},\"high\":{},\"medium\":{},\"low\":{},\"info\":{}",
            "}}}}\n"
        ),
        summary.scanned_sources(),
        summary.scanned_bytes(),
        summary.reports_with_findings(),
        summary.reports_without_findings(),
        summary.total_findings(),
        summary.total_candidates(),
        summary.reports_with_candidates(),
        summary.critical(),
        summary.high(),
        summary.medium(),
        summary.low(),
        summary.info(),
    )
    .expect("writing to String cannot fail");
    output
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use cribra::transform::TransformError;

    use super::*;

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
                    "cribra-preparation-{}-{nonce:x}-{sequence:x}",
                    std::process::id()
                ));
                match fs::create_dir(&root) {
                    Ok(()) => return Self { root },
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        panic!("temporary fixture directory should be created: {error}")
                    }
                }
            }
            panic!("temporary fixture directory should be available");
        }

        fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.root.join(name);
            fs::write(&path, bytes).unwrap();
            path
        }

        fn command(
            &self,
            mode: BundleMode,
            sources: Vec<PathBuf>,
            key_file: Option<PathBuf>,
        ) -> BundleCommand {
            BundleCommand {
                mode,
                sources,
                key_file,
                output_dir: self.root.join("uncreated").join("export"),
            }
        }

        fn assert_no_export(&self) {
            assert!(!self.root.join("uncreated").exists());
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn core_mode(mode: BundleMode, key: [u8; 32]) -> ShareMode {
        match mode {
            BundleMode::Redact => ShareMode::Redact,
            BundleMode::Template => ShareMode::Template,
            BundleMode::Pseudonymize => ShareMode::Pseudonymize(PseudonymizationOptions::new(key)),
            BundleMode::Synthesize => ShareMode::Synthesize(SynthesisOptions::new(key)),
        }
    }

    fn error(command: &BundleCommand) -> ExecuteError {
        // Prepared contents deliberately do not implement Debug.
        prepare(command).err().expect("preparation should fail")
    }

    #[test]
    fn all_modes_match_core_order_contents_and_exact_manifest_v1() {
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
        for (mode, name) in [
            (BundleMode::Redact, "redact"),
            (BundleMode::Template, "template"),
            (BundleMode::Pseudonymize, "pseudonymize"),
            (BundleMode::Synthesize, "synthesize"),
        ] {
            let command = fixture.command(
                mode,
                paths.clone(),
                matches!(mode, BundleMode::Pseudonymize | BundleMode::Synthesize)
                    .then(|| key_file.clone()),
            );
            let prepared = prepare(&command).unwrap();
            let scanner = Scanner::default();
            let core = ShareBundle::builder()
                .mode(core_mode(mode, key))
                .scan_and_build(
                    &scanner,
                    originals.iter().enumerate().map(|(i, source)| (i, *source)),
                )
                .unwrap();
            assert_eq!(prepared.sources.len(), 4);
            for (index, (source, expected)) in
                prepared.sources.iter().zip(core.sources()).enumerate()
            {
                assert_eq!(source.output_path, format!("sources/{:04}.txt", index + 1));
                assert!(source.content == expected.content());
                if index != 0 {
                    assert!(source.content == originals[index]);
                }
            }
            assert!(
                prepared.sources[0]
                    .content
                    .starts_with("\u{feff}π 😀\r\nGITHUB_TOKEN=")
            );
            assert!(
                prepared.sources[0]
                    .content
                    .ends_with("\r\nABCD-EFGH-IJKL-MNOP")
            );
            assert_eq!(prepared.sources[0].content.matches("\r\n").count(), 4);
            for value in [
                "ghp_AbCdEf0123456789_AbCdEf0123456789",
                "CorrectHorseBatteryStaple!",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ] {
                assert!(!prepared.sources[0].content.contains(value));
                assert!(!prepared.manifest_json.contains(value));
            }
            let expected = format!(
                concat!(
                    "{{\"schema_version\":1,\"mode\":\"{}\",",
                    "\"sources\":[\"sources/0001.txt\",\"sources/0002.txt\",\"sources/0003.txt\",\"sources/0004.txt\"],",
                    "\"summary\":{{\"scanned_sources\":4,\"scanned_bytes\":{},",
                    "\"reports_with_findings\":1,\"reports_without_findings\":3,",
                    "\"total_findings\":3,\"total_candidates\":2,\"reports_with_candidates\":2,",
                    "\"critical\":1,\"high\":1,\"medium\":1,\"low\":0,\"info\":0}}}}\n"
                ),
                name, scanned_bytes
            );
            assert_eq!(prepared.manifest_json, expected);
            for path in paths.iter().chain([&key_file, &command.output_dir]) {
                assert!(
                    !prepared
                        .manifest_json
                        .contains(path.to_string_lossy().as_ref())
                );
                assert!(
                    !prepared
                        .manifest_json
                        .contains(path.file_name().unwrap().to_string_lossy().as_ref())
                );
            }
            for forbidden in [
                "generated_at",
                "ABCD-EFGH-IJKL-MNOP",
                "cribra_pseudo_",
                "cribra_synthetic",
                "key",
                std::str::from_utf8(&key).unwrap(),
            ] {
                assert!(!prepared.manifest_json.contains(forbidden));
            }
            for (path, original) in paths.iter().zip(originals) {
                assert!(fs::read(path).unwrap() == original.as_bytes());
            }
            assert!(fs::read(&key_file).unwrap() == key);
            fixture.assert_no_export();
        }
    }

    #[test]
    fn manifest_is_deterministic_and_independent_of_key_and_transformed_identifiers() {
        let fixture = Fixture::new();
        let source = fixture.write(
            "source.env",
            b"GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789",
        );
        let first_key = fixture.write("first-key.bin", &[0; 32]);
        let second_key = fixture.write("second-key.bin", &[0xff; 32]);
        for mode in [BundleMode::Pseudonymize, BundleMode::Synthesize] {
            let mut command = fixture.command(mode, vec![source.clone()], Some(first_key.clone()));
            let first = prepare(&command).unwrap();
            let repeated = prepare(&command).unwrap();
            assert_eq!(first.manifest_json, repeated.manifest_json);
            assert!(first.sources[0].content == repeated.sources[0].content);
            command.key_file = Some(second_key.clone());
            let changed = prepare(&command).unwrap();
            assert_eq!(first.manifest_json, changed.manifest_json);
            assert!(first.sources[0].content != changed.sources[0].content);
        }
        fixture.assert_no_export();
    }

    #[test]
    fn keyed_preparation_validates_keys_before_any_source_acquisition() {
        let fixture = Fixture::new();
        let missing_source = fixture.root.join("missing.env");
        let missing_key = fixture.root.join("missing-key.bin");
        for mode in [BundleMode::Pseudonymize, BundleMode::Synthesize] {
            let command = fixture.command(
                mode,
                vec![missing_source.clone()],
                Some(missing_key.clone()),
            );
            assert!(matches!(error(&command), ExecuteError::KeyRead(_)));
            let command = fixture.command(mode, vec![Path::new("malformed\0/source").into()], None);
            assert!(matches!(error(&command), ExecuteError::KeyRead(_)));
            for bytes in [vec![], vec![0; 31], vec![0; 33], vec![0; 4096]] {
                let key = fixture.write("invalid-key.bin", &bytes);
                let command = fixture.command(mode, vec![missing_source.clone()], Some(key));
                assert!(matches!(error(&command), ExecuteError::InvalidKeyLength));
            }
            let command = fixture.command(
                mode,
                vec![missing_source.clone()],
                Some(fixture.root.clone()),
            );
            assert!(matches!(error(&command), ExecuteError::KeyRead(_)));
        }
        fixture.assert_no_export();
    }

    #[test]
    fn unkeyed_preparation_does_not_read_a_key() {
        let fixture = Fixture::new();
        let source = fixture.write("clean.txt", b"ordinary text");
        for mode in [BundleMode::Redact, BundleMode::Template] {
            let command =
                fixture.command(mode, vec![source.clone()], Some("malformed\0/key".into()));
            let prepared = prepare(&command).unwrap();
            assert!(prepared.sources[0].content == "ordinary text");
        }
        fixture.assert_no_export();
    }

    #[test]
    fn preparation_preserves_input_errors_without_panicking_on_paths() {
        let fixture = Fixture::new();
        let invalid_utf8 = fixture.write("invalid-utf8.bin", &[0xff]);
        let key = fixture.write("valid-key.bin", &[0; 32]);
        for mode in [
            BundleMode::Redact,
            BundleMode::Template,
            BundleMode::Pseudonymize,
            BundleMode::Synthesize,
        ] {
            let command = fixture.command(mode, vec![invalid_utf8.clone()], Some(key.clone()));
            assert!(matches!(error(&command), ExecuteError::InvalidUtf8));
            for path in [
                fixture.root.join("missing.env"),
                fixture.root.clone(),
                "malformed\0/source".into(),
            ] {
                let command = fixture.command(mode, vec![path], Some(key.clone()));
                assert!(matches!(error(&command), ExecuteError::InputRead(_)));
            }
        }
        assert!(fs::read(invalid_utf8).unwrap() == [0xff]);
        fixture.assert_no_export();
    }

    #[test]
    fn core_overlap_errors_propagate_without_output_side_effects() {
        let fixture = Fixture::new();
        let clean = fixture.write("first.txt", b"ordinary text");
        let source = "-----BEGIN PRIVATE KEY-----\nghp_AbCdEf0123456789_AbCdEf0123456789\n-----END PRIVATE KEY-----";
        let overlap = fixture.write("overlap.txt", source.as_bytes());
        let key = fixture.write("key.bin", &[7; 32]);
        for mode in [
            BundleMode::Template,
            BundleMode::Pseudonymize,
            BundleMode::Synthesize,
        ] {
            let command = fixture.command(
                mode,
                vec![clean.clone(), overlap.clone()],
                Some(key.clone()),
            );
            let error = error(&command);
            assert!(matches!(
                error,
                ExecuteError::Transform(TransformError::OverlappingSpans { .. })
            ));
            assert!(!error.to_string().contains("ghp_"));
            fixture.assert_no_export();
        }
        let command = fixture.command(BundleMode::Redact, vec![overlap.clone()], None);
        let prepared = prepare(&command).unwrap();
        assert!(prepared.sources[0].content == "[REDACTED]");
        assert!(fs::read(overlap).unwrap() == source.as_bytes());
        fixture.assert_no_export();
    }

    #[test]
    fn output_numbering_uses_minimum_width_without_truncation() {
        for (number, path) in [
            (1, "sources/0001.txt"),
            (9, "sources/0009.txt"),
            (9999, "sources/9999.txt"),
            (10000, "sources/10000.txt"),
            (10001, "sources/10001.txt"),
        ] {
            assert_eq!(source_output_path(number), path);
        }
    }
}
