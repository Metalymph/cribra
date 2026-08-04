#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Privacy-first Rust engine for detecting secrets and sensitive data.
//!
//! Silens Scan is a deterministic, local-first scanning core. It accepts UTF-8
//! text and returns structured findings without filesystem, network, terminal,
//! browser or cloud responsibilities.
//!
//! # Example
//!
//! ```
//! use silens_scan::{Rule, Scanner, Severity};
//!
//! let scanner = Scanner::builder()
//!     .rule(Rule::prefix(
//!         "example-token",
//!         "example_live_",
//!         Severity::Critical,
//!     ))
//!     .build()?;
//!
//! let report = scanner.scan("TOKEN=example_live_123456");
//! assert_eq!(report.len(), 1);
//!
//! # Ok::<(), silens_scan::ScannerBuildError>(())
//! ```

pub mod builtins;
mod compiled_rule;
mod confidence;
mod finding;
mod location;
mod redaction;
mod report;
mod rule;
mod scanner;
mod scanner_builder;
mod severity;
mod transform;
mod validators;

pub use confidence::Confidence;
pub use finding::Finding;
pub use location::Location;
pub use redaction::Redaction;
pub use report::ScanReport;
pub use rule::{Rule, RuleError, RuleId, RuleKind, RuleSpec};
pub use scanner::Scanner;
pub use scanner_builder::{ScannerBuildError, ScannerBuilder};
pub use severity::Severity;
