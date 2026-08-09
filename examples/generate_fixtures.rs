//! Regenerates the canonical fixture outputs committed under
//! `examples/fixtures/outputs`.
//!
//! This example intentionally writes files. Golden tests do not: they generate
//! actual values in memory and compare them against these committed artifacts.
//!
//! Run explicitly with:
//!
//! ```text
//! cargo run --example generate_fixtures --features serde
//! ```

#[cfg(not(feature = "serde"))]
fn main() {
    eprintln!("generate_fixtures requires the `serde` feature");
    std::process::exit(2);
}

#[cfg(feature = "serde")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    generate::run()
}

#[cfg(feature = "serde")]
mod generate {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use silens_scan::{
        Remediation, Rule, Scanner, Severity,
        transform::{
            PseudonymizationOptions, SynthesisOptions, pseudonymize, redact, synthesize, template,
        },
    };

    const PSEUDONYMIZATION_KEY: [u8; 32] = [0x31; 32];
    const SYNTHESIS_KEY: [u8; 32] = [0x53; 32];

    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let fixture_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/fixtures");
        let inputs = fixture_root.join("inputs");
        let outputs = fixture_root.join("outputs");

        prepare_output_directories(&outputs)?;

        let scanner = canonical_scanner()?;
        let mut input_files = input_files(&inputs)?;
        input_files.sort();

        for input_path in input_files {
            generate_for_input(&scanner, &inputs, &outputs, &input_path)?;
        }

        println!(
            "generated canonical outputs for {} fixture files",
            fs::read_dir(&inputs)?.count()
        );

        Ok(())
    }

    fn canonical_scanner() -> Result<Scanner, silens_scan::ScannerBuildError> {
        Scanner::builder()
            .rule(
                Rule::prefix(
                    "demo.api-key",
                    "demo_api_",
                    Severity::Critical,
                )
                .with_remediation(Remediation::RotateCredential),
            )
            .rule(
                Rule::pattern(
                    "demo.password",
                    r"demo-pass-[A-Za-z0-9_\-\p{L}]+",
                    Severity::High,
                )
                .expect("canonical password pattern must compile")
                .with_remediation(Remediation::RotatePassword),
            )
            .rule(
                Rule::literal(
                    "demo.private-key",
                    "DEMO_PRIVATE_KEY_MATERIAL",
                    Severity::Critical,
                )
                .with_remediation(Remediation::ReplacePrivateKey),
            )
            .rule(
                Rule::literal(
                    "demo.secret",
                    "DEMO_SECRET_ALPHA",
                    Severity::High,
                )
                .with_remediation(Remediation::RemoveSensitiveValue),
            )
            .build()
    }

    fn prepare_output_directories(outputs: &Path) -> std::io::Result<()> {
        for directory in [
            "reports",
            "redacted",
            "templates",
            "pseudonymized",
            "synthesized",
        ] {
            fs::create_dir_all(outputs.join(directory))?;
        }

        Ok(())
    }

    fn input_files(inputs: &Path) -> std::io::Result<Vec<PathBuf>> {
        fs::read_dir(inputs)?
            .filter_map(|entry| match entry {
                Ok(entry) if entry.file_type().is_ok_and(|kind| kind.is_file()) => {
                    Some(Ok(entry.path()))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    fn generate_for_input(
        scanner: &Scanner,
        inputs: &Path,
        outputs: &Path,
        input_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source = fs::read_to_string(input_path)?;
        let relative = input_path
            .strip_prefix(inputs)?
            .to_string_lossy()
            .replace('\\', "/");

        let results = scanner.scan([(relative.as_str(), source.as_str())]);
        let report = results
            .single_report()
            .expect("one input source must produce one report");

        let report_json = serde_json::to_string_pretty(&results)? + "\n";

        write_output(
            outputs,
            "reports",
            &format!("{relative}.json"),
            report_json.as_bytes(),
        )?;
        write_output(
            outputs,
            "redacted",
            &relative,
            redact(&source, report)?.as_bytes(),
        )?;
        write_output(
            outputs,
            "templates",
            &relative,
            template(&source, report)?.as_bytes(),
        )?;
        write_output(
            outputs,
            "pseudonymized",
            &relative,
            pseudonymize(
                &source,
                report,
                &PseudonymizationOptions::new(PSEUDONYMIZATION_KEY),
            )?
            .as_bytes(),
        )?;
        write_output(
            outputs,
            "synthesized",
            &relative,
            synthesize(
                &source,
                report,
                &SynthesisOptions::new(SYNTHESIS_KEY),
            )?
            .as_bytes(),
        )?;

        Ok(())
    }

    fn write_output(
        outputs: &Path,
        category: &str,
        relative: &str,
        content: &[u8],
    ) -> std::io::Result<()> {
        let destination = outputs.join(category).join(relative);

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(destination, content)
    }
}
