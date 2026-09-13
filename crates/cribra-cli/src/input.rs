//! UTF-8 input handling for the canonical Cribra CLI.
//!
//! This module owns only command-line input acquisition. Filesystem traversal,
//! discovery, globbing, repository walking, and encoding conversion remain out
//! of scope.

use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

/// Explicit source selected by the command-line caller.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Input {
    /// Read one explicit filesystem path.
    File(PathBuf),

    /// Read bytes from standard input.
    Stdin,
}

/// Fully acquired UTF-8 input.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SourceInput {
    name: String,
    text: String,
}

impl SourceInput {
    /// Returns the presentation-safe source name.
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Returns the exact UTF-8 source text.
    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

/// Input acquisition failure.
#[derive(Debug)]
pub(crate) enum InputError {
    Io(io::Error),
    InvalidUtf8,
}

impl std::fmt::Display for InputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::InvalidUtf8 => formatter.write_str("input is not valid UTF-8"),
        }
    }
}

impl std::error::Error for InputError {}

/// Reads one explicit CLI input as UTF-8 without normalization.
///
/// No newline conversion, BOM stripping, lossy decoding, or encoding
/// auto-detection is performed.
pub(crate) fn read(input: &Input) -> Result<SourceInput, InputError> {
    match input {
        Input::File(path) => read_file(path),
        Input::Stdin => read_stdin(),
    }
}

fn read_file(path: &Path) -> Result<SourceInput, InputError> {
    let bytes = fs::read(path).map_err(InputError::Io)?;
    source_from_bytes(path.to_string_lossy().into_owned(), bytes)
}

fn read_stdin() -> Result<SourceInput, InputError> {
    let mut bytes = Vec::new();

    io::stdin()
        .read_to_end(&mut bytes)
        .map_err(InputError::Io)?;

    source_from_bytes("<stdin>".to_owned(), bytes)
}

fn source_from_bytes(name: String, bytes: Vec<u8>) -> Result<SourceInput, InputError> {
    let text = String::from_utf8(bytes).map_err(|_| InputError::InvalidUtf8)?;

    Ok(SourceInput { name, text })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_input_preserves_text_exactly() {
        let source = SourceInput {
            name: "fixture".to_owned(),
            text: "α\r\nTOKEN=value\n".to_owned(),
        };

        assert_eq!(source.name(), "fixture");
        assert_eq!(source.text(), "α\r\nTOKEN=value\n");
    }

    #[test]
    fn empty_utf8_input_is_preserved() {
        let source = source_from_bytes("empty".to_owned(), Vec::new()).unwrap();

        assert_eq!(source.name(), "empty");
        assert_eq!(source.text(), "");
    }

    #[test]
    fn crlf_input_is_preserved_without_normalization() {
        let source =
            source_from_bytes("fixture".to_owned(), b"first\r\nsecond\r\n".to_vec()).unwrap();

        assert_eq!(source.text().as_bytes(), b"first\r\nsecond\r\n");
    }

    #[test]
    fn utf8_input_is_preserved_exactly() {
        let text = "π 😀 café\nTOKEN=value\n";

        let source = source_from_bytes("unicode".to_owned(), text.as_bytes().to_vec()).unwrap();

        assert_eq!(source.text(), text);
        assert_eq!(source.text().as_bytes(), text.as_bytes());
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let error =
            source_from_bytes("invalid".to_owned(), vec![0xf0, 0x28, 0x8c, 0x28]).unwrap_err();

        assert!(matches!(error, InputError::InvalidUtf8));
    }

    #[test]
    fn utf8_bom_is_not_silently_removed() {
        let bytes = vec![0xef, 0xbb, 0xbf, b'T', b'O', b'K', b'E', b'N'];

        let source = source_from_bytes("bom".to_owned(), bytes).unwrap();

        assert_eq!(source.text(), "\u{feff}TOKEN");
    }

    #[test]
    fn explicit_file_input_reads_exact_bytes_as_utf8() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "cribra-cli-input-{}-{unique}.txt",
            std::process::id()
        ));

        let expected = "π\r\nTOKEN=value\r\n";

        fs::write(&path, expected.as_bytes()).unwrap();

        let result = read(&Input::File(path.clone())).unwrap();

        fs::remove_file(&path).unwrap();

        assert_eq!(result.name(), path.to_string_lossy());
        assert_eq!(result.text().as_bytes(), expected.as_bytes());
    }

    #[test]
    fn missing_file_returns_io_error() {
        let path = std::env::temp_dir().join(format!(
            "cribra-cli-definitely-missing-{}",
            std::process::id()
        ));

        let _ = fs::remove_file(&path);

        let error = read(&Input::File(path)).unwrap_err();

        assert!(matches!(error, InputError::Io(_)));
    }
}
