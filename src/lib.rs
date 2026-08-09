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
//! let results = scanner.scan([
//!     ("memory", "TOKEN=example_live_123456"),
//! ]);
//!
//! let report = results.single_report().expect("one source was scanned");
//! assert_eq!(report.len(), 1);
//!
//! # Ok::<(), silens_scan::ScannerBuildError>(())
//! ```
//!
//! With the optional `parallel` feature, the same batch can be scanned with
//! [`Scanner::parallel_scan`] while preserving input order.

pub mod builtins;
mod compiled_rule;
mod confidence;
mod finding;
mod location;
mod redaction;
mod remediation;
mod report;
mod rule;
mod scan_entry;
mod scan_query;
mod scan_results;
mod scan_sort;
mod scan_summary;
mod scanner;
mod scanner_builder;
mod severity;
pub mod transform;
mod validators;

pub use confidence::Confidence;
pub use finding::Finding;
pub use location::Location;
pub use redaction::Redaction;
pub use remediation::Remediation;
pub use report::ScanReport;
pub use rule::{Rule, RuleError, RuleId, RuleKind, RuleSpec};
pub use scan_entry::ScanEntry;
pub use scan_query::{ScanQuery, SortedScanQuery};
pub use scan_results::ScanResults;
pub use scan_sort::ScanSort;
pub use scan_summary::ScanSummary;
pub use scanner::Scanner;
pub use scanner_builder::{ScannerBuildError, ScannerBuilder};
pub use severity::Severity;
pub use transform::redact;
