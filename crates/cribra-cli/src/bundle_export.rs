//! Bundle execution boundary. Unsupported platforms never acquire inputs.

use crate::{BundleCommand, ExecuteError};

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn export(command: &BundleCommand) -> Result<(), ExecuteError> {
    validate(command)?;
    let prepared = crate::bundle::prepare(command)?;
    unix::publish(&command.output_dir, &prepared).map_err(ExecuteError::BundleExport)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) fn export(_: &BundleCommand) -> Result<(), ExecuteError> {
    Err(ExecuteError::UnsupportedOperation(
        "bundle export is supported only on macOS and Linux",
    ))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn validate(command: &BundleCommand) -> Result<(), ExecuteError> {
    use crate::BundleMode;
    fn valid_path(path: &std::path::Path) -> bool {
        !path.as_os_str().is_empty() && !path.as_os_str().as_encoded_bytes().starts_with(b"-")
    }
    let invalid = ExecuteError::InvalidBundleCommand;
    if command.sources.is_empty() {
        return Err(invalid("at least one explicit source file is required"));
    }
    if !valid_path(&command.output_dir) || command.sources.iter().any(|path| !valid_path(path)) {
        return Err(invalid(
            "nonempty filesystem paths are required; stdin and option-like paths are unsupported",
        ));
    }
    for (index, source) in command.sources.iter().enumerate() {
        if command.sources[..index].contains(source) {
            return Err(invalid("duplicate source paths are unsupported"));
        }
    }
    if matches!(
        command.mode,
        BundleMode::Pseudonymize | BundleMode::Synthesize
    ) {
        let key = command
            .key_file
            .as_deref()
            .ok_or_else(|| invalid("keyed modes require a key file"))?;
        if !valid_path(key) {
            return Err(invalid("a nonempty key filesystem path is required"));
        }
    } else if command.key_file.is_some() {
        return Err(invalid("unkeyed modes do not accept a key file"));
    }
    Ok(())
}
