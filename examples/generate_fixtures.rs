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
#[path = "fixtures/corpus.rs"]
mod corpus;

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

    use cribra::{
        Scanner,
        transform::{
            PseudonymizationOptions, SynthesisOptions, pseudonymize, redact, synthesize, template,
        },
    };

    use super::corpus;

    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/fixtures");
        let inputs = fixture_root.join("inputs");
        let outputs = fixture_root.join("outputs");

        prepare_output_directories(&outputs)?;

        let scanner = corpus::scanner()?;
        let input_files = sorted_input_files(&inputs)?;

        for input_path in &input_files {
            generate_for_input(&scanner, &inputs, &outputs, input_path)?;
        }

        println!(
            "generated canonical outputs for {} fixture files",
            input_files.len(),
        );

        Ok(())
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

    fn sorted_input_files(inputs: &Path) -> std::io::Result<Vec<PathBuf>> {
        let mut files = fs::read_dir(inputs)?
            .filter_map(|entry| match entry {
                Ok(entry) if entry.file_type().is_ok_and(|kind| kind.is_file()) => {
                    Some(Ok(entry.path()))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<std::io::Result<Vec<_>>>()?;

        files.sort();
        Ok(files)
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
                &PseudonymizationOptions::new(corpus::PSEUDONYMIZATION_KEY),
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
                &SynthesisOptions::new(corpus::SYNTHESIS_KEY),
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
