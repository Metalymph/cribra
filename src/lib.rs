//! Privacy-first scanning core for secrets and sensitive data.
//!
//! `cribra` provides deterministic detection, reporting, querying and
//! share-safe transformation of UTF-8 text. Applications own I/O and storage;
//! the crate operates on caller-provided text and does not retain matched
//! secret values inside public [`Finding`] values.
//!
//! # Quick start
//!
//! ```
//! use cribra::Scanner;
//!
//! let scanner = Scanner::default();
//! let results = scanner.scan([
//!     ("config.env", "TOKEN=example"),
//!     ("settings.toml", "mode = \"production\""),
//! ]);
//!
//! assert_eq!(results.len(), 2);
//! println!("{}", results.summary());
//! ```
//!
//! # Result model
//!
//! A scan returns [`ScanResults<K>`], preserving the caller's source key `K`.
//! Each source owns an immutable [`ScanReport`], whose [`Finding`] values expose
//! rule metadata, severity, confidence, optional [`Remediation`] and a
//! [`Location`].
//!
//! Source coordinates use:
//!
//! - zero-based, half-open UTF-8 byte offsets;
//! - one-based lines;
//! - one-based Unicode scalar columns.
//!
//! Findings intentionally do not contain the matched source value.
//!
//! //! # Ambiguous candidates and explainability
//!
//! [`ScanReport`] keeps classified [`Finding`] values separate from
//! [`SensitiveCandidate`] values that are structurally review-worthy but do not
//! have enough evidence for classification.
//!
//! Explainability projects those existing authorities into [`Explanation`]:
//!
//! - `Explanation::Classified(DetectionMode)` describes how a rule-backed
//!   finding was validated;
//! - `Explanation::Ambiguous(CandidateEvidence)` describes the evidence behind
//!   a review-only candidate.
//!
//! Findings do not duplicate rule metadata. Their explanation is resolved
//! against the [`Scanner`] that owns the compiled metadata and fails closed when
//! it cannot be resolved unambiguously. Candidate explanation is projected
//! directly from its existing evidence.
//!
//! Explanation is presentation-agnostic and contains no source snippets or
//! matched sensitive values. Applications remain responsible for human-facing
//! copy.
//!
//! # Querying
//!
//! [`ScanResults::query`] builds a lazy [`ScanQuery`] over borrowed findings.
//! Filters can be composed before optionally materializing an explicitly sorted
//! [`SortedScanQuery`].
//!
//! ```
//! use cribra::{ScanSort, Scanner, Severity};
//!
//! let scanner = Scanner::default();
//! let results = scanner.scan([("config.env", "TOKEN=example")]);
//!
//! let findings = results
//!     .query()
//!     .minimum_severity(Severity::High)
//!     .sort(ScanSort::Location);
//!
//! for (source, finding) in findings.iter() {
//!     println!("{source}: {}", finding.rule_id());
//! }
//! ```
//!
//! # Transformations
//!
//! [`transform`] provides explicit share-safe transformations:
//!
//! - [`transform::redact`] for conservative replacement;
//! - [`transform::template`] for semantic placeholders;
//! - [`transform::pseudonymize`] for deterministic keyed pseudonyms;
//! - [`transform::synthesize`] for deterministic keyed synthetic values;
//! - [`transform::ShareBundle`] for transformed keyed batches plus manifest
//!   metadata.
//!
//! ```
//! use cribra::{Rule, Scanner, Severity, transform::redact};
//!
//! let scanner = Scanner::builder()
//!     .rule(Rule::literal("credential", "SECRET", Severity::High))
//!     .build()?;
//!
//! let source = "TOKEN=SECRET";
//! let results = scanner.scan([("memory", source)]);
//! let report = results.single_report().expect("one report");
//!
//! assert_eq!(redact(source, report)?, "TOKEN=[REDACTED]");
//!
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Optional features
//!
//! `serde` enables serialization support for public data contracts.
//!
//! `parallel` enables `Scanner::parallel_scan`, which distributes independent
//! inputs through Rayon while preserving input order and the same per-source
//! semantics as serial scanning.
//!
//! # Application boundary
//!
//! File loading, network access, repository integration, authentication,
//! persistence and UI are intentionally outside this crate. This keeps the
//! scanner reusable in local-first native, WASM/PWA, desktop and service
//! applications.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Privacy-first Rust engine for detecting secrets and sensitive data.
//!
//! Cribrais a deterministic, local-first scanning core. It accepts UTF-8
//! text and returns structured findings without filesystem, network, terminal,
//! browser or cloud responsibilities.
//!
//! # Example
//!
//! ```
//! use cribra::{Rule, Scanner, Severity};
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
//! # Ok::<(), cribra::ScannerBuildError>(())
//! ```
//!
//! With the optional `parallel` feature, native callers can use
//! `Scanner::parallel_scan` while preserving input order.
pub mod builtins;
mod candidate_detection;
mod compiled_rule;
mod confidence;
mod explanation;
mod finding;
mod location;
mod redaction;
mod remediation;
mod rule;
mod rule_metadata;
mod scan_entry;
mod scan_query;
mod scan_report;
mod scan_results;
mod scan_sort;
mod scan_summary;
mod scanner;
mod scanner_builder;
mod sensitive_candidate;
mod severity;
pub mod transform;
mod validators;
pub use confidence::Confidence;
pub use explanation::Explanation;
pub use finding::Finding;
pub use location::Location;
pub use redaction::Redaction;
pub use remediation::Remediation;
pub use rule::{Rule, RuleError, RuleId, RuleKind, RuleSpec};
pub use rule_metadata::{DetectionMode, RuleMetadata};
pub use scan_entry::ScanEntry;
pub use scan_query::{ScanQuery, SortedScanQuery};
pub use scan_report::ScanReport;
pub use scan_results::ScanResults;
pub use scan_sort::ScanSort;
pub use scan_summary::ScanSummary;
pub use scanner::Scanner;
pub use scanner_builder::{ScannerBuildError, ScannerBuilder};
pub use sensitive_candidate::{CandidateEvidence, SensitiveCandidate, SensitiveCandidateKind};
pub use severity::Severity;
pub use transform::redact;
