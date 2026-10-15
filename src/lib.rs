#![forbid(unsafe_code)]
#![warn(missing_docs)]

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
//! # Ambiguous candidates and explainability
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
//! The default-enabled `transform` feature provides explicit share-safe transformations:
//!
//! - `transform::redact` for conservative replacement;
//! - `transform::template` for semantic placeholders;
//! - `transform::pseudonymize` for deterministic keyed pseudonyms;
//! - `transform::synthesize` for deterministic keyed synthetic values;
//! - `transform::ShareBundle` for transformed keyed batches plus manifest
//!   metadata.
//!
//! ```
//! # #[cfg(feature = "transform")]
//! # {
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
//! # }
//!
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Optional features
//!
//! Minimal (`default-features = false`) includes scanning, built-ins, reporting,
//! streaming and incremental sessions, with the standalone [`Redaction`] type.
//! Standard (the default) adds `transform`, including keyed transformations backed
//! by BLAKE3. Full adds `parallel` and `serde` to Standard.
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
//!
//! # Execution models
//!
//! [`Scanner::scan`] is the whole-source semantic reference. Published v0.5.0
//! supports [`Scanner::stream`] for explicitly admitted configurations, with
//! bounded retained source history and successful-EOF-only complete reports.
//! Complete built-in portfolios remain unsupported for streaming.
//!
//! The implemented, unreleased v0.6.0 [`IncrementalSession`] owns a revisioned
//! UTF-8 document and maintains exact complete reports after replacement edits.
//! Literal and structural matching use local invalidation; Prefix, Suffix and
//! regex rules use authoritative whole-rule fallback. This is mutable-document
//! rescanning, distinct from append-only streaming.
//!
//! Optional limits account for controlled logical payload and result ownership,
//! not total heap memory. Incremental resource/execution failures preserve the
//! last complete snapshot and prevent further edits. Input errors are recoverable.
//! Session APIs are Rust-core surfaces; current adapters do not expose them.
pub mod builtins;
mod candidate_detection;
mod compiled_rule;
mod confidence;
mod explanation;
mod finding;
mod incremental_session;
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
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "wired incrementally during streaming 0.5-D")
)]
mod source_coordinates;
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "wired incrementally during streaming 0.5-D")
)]
mod source_session;
mod stream;
#[cfg(feature = "transform")]
pub mod transform;
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "wired incrementally during streaming 0.5-D")
)]
mod utf8_transport;
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
pub use stream::{StreamError, StreamLimits, StreamSession};
#[cfg(feature = "transform")]
pub use transform::redact;

pub use incremental_session::{IncrementalError, IncrementalLimits, IncrementalSession};
