use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::*;
use crate::{FindingSeverity, ScanEngineBuilder};
use cribra::transform::{
    PseudonymizationOptions, ShareBundle as CoreShareBundle, ShareMode as CoreShareMode,
    ShareModeKind as CoreShareModeKind, SynthesisOptions, TransformError,
};

fn literal_engine() -> ScanEngine {
    let mut builder = ScanEngineBuilder::new(false);
    builder.add_literal("custom.secret", "SECRET", FindingSeverity::High);
    builder.build().expect("literal scanner should build")
}

fn core_share<'a>(
    engine: &ScanEngine,
    mode: CoreShareMode,
    inputs: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> CoreShareBundle<&'a str> {
    CoreShareBundle::builder()
        .mode(mode)
        .scan_and_build(&engine.scanner, inputs)
        .expect("direct Core share bundle should build")
}

fn redacted_builder() -> ShareBundleBuilder {
    let mut builder = ShareBundleBuilder::new();
    builder.mode_redact();
    builder
}

fn assert_manifest_mode(bundle: &ShareBundle, expected: ShareModeKind) {
    let manifest = bundle.manifest();
    assert_eq!(manifest.mode(), expected);
    assert!(manifest.generated_at_nanoseconds() < 1_000_000_000);
}

#[test]
fn empty_source_collection_produces_clean_empty_bundle() {
    let engine = literal_engine();
    let bundle = redacted_builder()
        .scan_and_build_core(&engine, Vec::new())
        .unwrap();

    assert_eq!(bundle.length(), 0);
    assert!(bundle.is_empty());
    assert_manifest_mode(&bundle, ShareModeKind::Redact);

    let manifest = bundle.manifest();
    let summary = manifest.summary();
    assert_eq!(summary.scanned_sources(), 0);
    assert_eq!(summary.scanned_bytes(), 0);
    assert_eq!(summary.total_findings(), 0);
    assert_eq!(summary.total_candidates(), 0);
    assert!(summary.is_clean());
    assert!(!summary.has_critical());
    assert!(bundle.entry_at_core(0).is_none());
}

#[test]
fn one_source_matches_direct_core_bundle_and_manifest_summary() {
    let engine = literal_engine();
    let source = "é SECRET";
    let mut builder = ShareBundleBuilder::new();
    builder.mode_redact();
    let wasm = builder
        .scan_and_build_core(&engine, vec![("notes.txt".into(), source.into())])
        .unwrap();
    let core = core_share(&engine, CoreShareMode::Redact, [("notes.txt", source)]);

    assert_eq!(wasm.length(), 1);
    assert_eq!(wasm.manifest().mode(), ShareModeKind::Redact);
    assert_eq!(core.manifest().mode(), CoreShareModeKind::Redact);
    assert_eq!(wasm.entry_at(0).unwrap().id(), "notes.txt");
    assert_eq!(
        wasm.entry_at(0).unwrap().content(),
        core.sources()[0].content()
    );
    assert_eq!(core.sources()[0].content(), "é [REDACTED]");

    let summary = wasm.summary();
    let core_summary = core.summary();
    assert_eq!(summary.scanned_sources(), core_summary.scanned_sources());
    assert_eq!(summary.scanned_bytes(), core_summary.scanned_bytes());
    assert_eq!(summary.total_findings(), core_summary.total_findings());
    assert_eq!(summary.total_candidates(), core_summary.total_candidates());
    assert_eq!(summary.has_critical(), core_summary.has_critical());
}

#[test]
fn multiple_sources_preserve_core_order_duplicate_and_empty_ids() {
    let engine = literal_engine();
    let inputs = vec![
        ("same".to_owned(), "first SECRET".to_owned()),
        ("".to_owned(), "clean".to_owned()),
        ("same".to_owned(), "🦀 SECRET\nSECRET".to_owned()),
    ];
    let bundle = redacted_builder()
        .scan_and_build_core(&engine, inputs.clone())
        .unwrap();

    assert_eq!(bundle.length(), 3);
    assert_eq!(bundle.entry_at(0).unwrap().id(), "same");
    assert_eq!(bundle.entry_at(1).unwrap().id(), "");
    assert_eq!(bundle.entry_at(2).unwrap().id(), "same");
    assert_eq!(bundle.entry_at(0).unwrap().content(), "first [REDACTED]");
    assert_eq!(bundle.entry_at(1).unwrap().content(), "clean");
    assert_eq!(
        bundle.entry_at(2).unwrap().content(),
        "🦀 [REDACTED]\n[REDACTED]"
    );
    assert_eq!(bundle.summary().scanned_sources(), 3);
    assert_eq!(bundle.summary().total_findings(), 3);
}

#[test]
fn unicode_ids_and_utf8_contents_are_preserved() {
    let engine = literal_engine();
    let source = "前🦀 é SECRET";
    let bundle = redacted_builder()
        .scan_and_build_core(&engine, vec![("独立/🦀".into(), source.into())])
        .unwrap();
    let entry = bundle.entry_at(0).unwrap();

    assert_eq!(entry.id(), "独立/🦀");
    assert_eq!(entry.content(), "前🦀 é [REDACTED]");
    assert_eq!(bundle.summary().scanned_bytes(), source.len());
    assert_eq!(bundle.summary().total_findings(), 1);
}

#[test]
fn clean_sources_remain_byte_for_byte_unchanged() {
    let engine = literal_engine();
    let source = "ordinary UTF-8 text: é🦀\nno findings";
    let bundle = redacted_builder()
        .scan_and_build_core(&engine, vec![("clean".into(), source.into())])
        .unwrap();

    assert_eq!(bundle.entry_at(0).unwrap().content(), source);
    assert_eq!(bundle.summary().total_findings(), 0);
    assert!(bundle.summary().is_clean());
}

#[test]
fn classified_findings_are_transformed() {
    let engine = literal_engine();
    let bundle = redacted_builder()
        .scan_and_build_core(&engine, vec![("finding".into(), "TOKEN=SECRET".into())])
        .unwrap();

    assert_eq!(bundle.entry_at(0).unwrap().content(), "TOKEN=[REDACTED]");
    assert_eq!(bundle.summary().total_findings(), 1);
    assert!(!bundle.summary().is_clean());
}

#[test]
fn ambiguous_candidates_remain_review_only_and_untransformed() {
    let engine = ScanEngine::new();
    let source = "ABCD-EFGH-IJKL-MNOP";
    let bundle = redacted_builder()
        .scan_and_build_core(&engine, vec![("review".into(), source.into())])
        .unwrap();

    assert_eq!(bundle.entry_at(0).unwrap().content(), source);
    assert_eq!(bundle.summary().total_findings(), 0);
    assert_eq!(bundle.summary().total_candidates(), 1);
    assert!(bundle.summary().has_candidates());
    assert!(!bundle.summary().is_clean());
}

#[test]
fn template_mode_uses_core_default_template_policy() {
    let engine = literal_engine();
    let mut builder = ShareBundleBuilder::new();
    builder.mode_template();
    let bundle = builder
        .scan_and_build_core(&engine, vec![("one".into(), "TOKEN=SECRET".into())])
        .unwrap();
    let core = core_share(&engine, CoreShareMode::Template, [("one", "TOKEN=SECRET")]);

    assert_eq!(bundle.manifest().mode(), ShareModeKind::Template);
    assert_eq!(
        bundle.entry_at(0).unwrap().content(),
        "TOKEN=<CRIBRA:custom.secret>"
    );
    assert_eq!(
        bundle.entry_at(0).unwrap().content(),
        core.sources()[0].content()
    );
}

#[test]
fn pseudonymization_configuration_and_defaults_match_core() {
    let engine = literal_engine();
    let key = [7_u8; 32];
    let source = "A=SECRET B=SECRET";

    let mut configured = ShareBundleBuilder::new();
    configured
        .mode_pseudonymize(&key, Some("pseudo:".into()), Some(8))
        .unwrap();
    let wasm = configured
        .scan_and_build_core(&engine, vec![("one".into(), source.into())])
        .unwrap();
    let core = core_share(
        &engine,
        CoreShareMode::Pseudonymize(
            PseudonymizationOptions::new(key)
                .prefix("pseudo:")
                .digest_bytes(8),
        ),
        [("one", source)],
    );
    assert_eq!(
        wasm.entry_at(0).unwrap().content(),
        core.sources()[0].content()
    );
    assert!(wasm.entry_at(0).unwrap().content().contains("pseudo:"));

    let mut defaults = ShareBundleBuilder::new();
    defaults.mode_pseudonymize(&key, None, None).unwrap();
    let wasm_default = defaults
        .scan_and_build_core(&engine, vec![("one".into(), source.into())])
        .unwrap();
    let core_default = core_share(
        &engine,
        CoreShareMode::Pseudonymize(PseudonymizationOptions::new(key)),
        [("one", source)],
    );
    assert_eq!(
        wasm_default.entry_at(0).unwrap().content(),
        core_default.sources()[0].content()
    );
    assert!(
        wasm_default
            .entry_at(0)
            .unwrap()
            .content()
            .contains("cribra_pseudo_")
    );
}

#[test]
fn pseudonymization_digest_length_is_clamped_by_core_options() {
    let engine = literal_engine();
    let key = [3_u8; 32];
    let source = "SECRET";
    let mut builder = ShareBundleBuilder::new();
    builder
        .mode_pseudonymize(&key, Some(String::new()), Some(2))
        .unwrap();
    let wasm = builder
        .scan_and_build_core(&engine, vec![("one".into(), source.into())])
        .unwrap();
    let core = core_share(
        &engine,
        CoreShareMode::Pseudonymize(
            PseudonymizationOptions::new(key)
                .prefix(String::new())
                .digest_bytes(2),
        ),
        [("one", source)],
    );

    assert_eq!(
        wasm.entry_at(0).unwrap().content(),
        core.sources()[0].content()
    );
    assert_eq!(wasm.entry_at(0).unwrap().content().len(), 16);
}

#[test]
fn synthesis_configuration_and_defaults_match_core() {
    let engine = literal_engine();
    let key = [9_u8; 32];
    let source = "TOKEN=SECRET";

    let mut configured = ShareBundleBuilder::new();
    configured
        .mode_synthesize(&key, Some("fixture".into()))
        .unwrap();
    let wasm = configured
        .scan_and_build_core(&engine, vec![("one".into(), source.into())])
        .unwrap();
    let core = core_share(
        &engine,
        CoreShareMode::Synthesize(SynthesisOptions::new(key).marker("fixture")),
        [("one", source)],
    );
    assert_eq!(
        wasm.entry_at(0).unwrap().content(),
        core.sources()[0].content()
    );

    let mut defaults = ShareBundleBuilder::new();
    defaults.mode_synthesize(&key, None).unwrap();
    let wasm_default = defaults
        .scan_and_build_core(&engine, vec![("one".into(), source.into())])
        .unwrap();
    let core_default = core_share(
        &engine,
        CoreShareMode::Synthesize(SynthesisOptions::new(key)),
        [("one", source)],
    );
    assert_eq!(
        wasm_default.entry_at(0).unwrap().content(),
        core_default.sources()[0].content()
    );
    assert_eq!(wasm_default.manifest().mode(), ShareModeKind::Synthesize);
}

#[test]
fn missing_mode_preserves_core_failure_and_returns_no_bundle() {
    let engine = literal_engine();
    let builder = ShareBundleBuilder::new();
    let error = builder
        .scan_and_build_core(&engine, vec![("one".into(), "SECRET".into())])
        .unwrap_err();

    assert_eq!(
        error,
        ShareBundleFailure::Transform(TransformError::MissingShareMode)
    );
    assert_eq!(
        TransformError::MissingShareMode.to_string(),
        "share bundle transformation mode was not configured"
    );
}

#[test]
fn keyed_mode_selection_rejects_non_32_byte_keys_without_staging_mode() {
    let mut builder = ShareBundleBuilder::new();
    assert!(
        builder
            .mode_pseudonymize_core(&[0_u8; 31], None, None)
            .is_err()
    );
    assert!(
        builder
            .mode_pseudonymize_core(&[0_u8; 33], None, None)
            .is_err()
    );
    assert!(builder.mode_synthesize_core(&[0_u8; 31], None).is_err());
    assert!(builder.mode_synthesize_core(&[0_u8; 33], None).is_err());

    let engine = literal_engine();
    assert_eq!(
        builder
            .scan_and_build_core(&engine, vec![("one".into(), "SECRET".into())])
            .unwrap_err(),
        ShareBundleFailure::Transform(TransformError::MissingShareMode)
    );
}

#[test]
fn host_millisecond_clock_conversion_is_exact_and_fail_closed() {
    let expected =
        UNIX_EPOCH + Duration::from_secs(1_700_000_000) + Duration::from_nanos(123_000_000);

    assert_eq!(system_time_from_unix_millis(0.0), Some(UNIX_EPOCH));
    assert_eq!(
        system_time_from_unix_millis(1_700_000_000_123.0),
        Some(expected)
    );
    assert_eq!(
        system_time_from_unix_millis(1_700_000_000_123.999),
        Some(expected)
    );
    assert_eq!(system_time_from_unix_millis(-1.0), None);
    assert_eq!(system_time_from_unix_millis(f64::NAN), None);
    assert_eq!(system_time_from_unix_millis(f64::INFINITY), None);
    assert_eq!(system_time_from_unix_millis(9_007_199_254_740_992.0), None);
}

#[test]
fn explicit_core_generation_timestamp_projects_seconds_and_nanoseconds() {
    let engine = literal_engine();
    let generated_at =
        UNIX_EPOCH + Duration::from_secs(1_700_000_000) + Duration::from_nanos(987_654_321);
    let bundle = redacted_builder()
        .scan_and_build_at_core(&engine, vec![("one".into(), "SECRET".into())], generated_at)
        .unwrap();
    let manifest = bundle.manifest();

    assert_eq!(manifest.generated_at_seconds(), 1_700_000_000);
    assert_eq!(manifest.generated_at_nanoseconds(), 987_654_321);
    assert_eq!(manifest.mode(), ShareModeKind::Redact);
    assert_eq!(manifest.summary().total_findings(), 1);
}

#[test]
fn manifest_uses_core_timestamp_and_deterministic_fields() {
    let engine = literal_engine();
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let bundle = redacted_builder()
        .scan_and_build_core(&engine, vec![("one".into(), "SECRET".into())])
        .unwrap();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let manifest = bundle.manifest();

    assert_eq!(manifest.mode(), ShareModeKind::Redact);
    assert!(manifest.generated_at_seconds() >= before);
    assert!(manifest.generated_at_seconds() <= after);
    assert!(manifest.generated_at_nanoseconds() < 1_000_000_000);
    assert_eq!(manifest.summary().scanned_sources(), 1);
    assert_eq!(manifest.summary().total_findings(), 1);
}

#[test]
fn bundle_results_outlive_engine_builder_and_bundle() {
    let engine = literal_engine();
    let builder = redacted_builder();
    let bundle = builder
        .scan_and_build_core(&engine, vec![("独立".into(), "🦀 SECRET".into())])
        .unwrap();
    let entry = bundle.entry_at(0).unwrap();
    let manifest = bundle.manifest();
    let summary = bundle.summary();

    drop(builder);
    drop(engine);
    drop(bundle);

    assert_eq!(entry.id(), "独立");
    assert_eq!(entry.content(), "🦀 [REDACTED]");
    assert_eq!(manifest.mode(), ShareModeKind::Redact);
    assert_eq!(manifest.summary().total_findings(), 1);
    assert_eq!(summary.total_findings(), 1);
    assert!(manifest.generated_at_nanoseconds() < 1_000_000_000);
}

#[test]
fn repeated_bundle_construction_returns_independent_owned_results() {
    let engine = literal_engine();
    let builder = redacted_builder();
    let first = builder
        .scan_and_build_core(&engine, vec![("same".into(), "SECRET".into())])
        .unwrap();
    let second = builder
        .scan_and_build_core(&engine, vec![("same".into(), "SECRET".into())])
        .unwrap();
    let first_entry = first.entry_at(0).unwrap();
    let second_entry = second.entry_at(0).unwrap();
    let first_manifest = first.manifest();
    let second_manifest = second.manifest();

    drop(first);
    assert_eq!(second_entry.content(), "[REDACTED]");
    assert_eq!(second_manifest.mode(), ShareModeKind::Redact);
    drop(second);
    assert_eq!(first_entry.id(), "same");
    assert_eq!(first_entry.content(), "[REDACTED]");
    assert_eq!(first_manifest.summary().total_findings(), 1);
}

#[test]
fn last_selected_mode_wins_and_mode_changes_do_not_retain_source() {
    let engine = literal_engine();
    let mut builder = ShareBundleBuilder::new();
    builder.mode_synthesize(&[4_u8; 32], None).unwrap();
    builder.mode_redact();
    let bundle = builder
        .scan_and_build_core(&engine, vec![("one".into(), "TOKEN=SECRET".into())])
        .unwrap();

    assert_eq!(bundle.manifest().mode(), ShareModeKind::Redact);
    assert_eq!(bundle.entry_at(0).unwrap().content(), "TOKEN=[REDACTED]");
    assert!(!bundle.entry_at(0).unwrap().content().contains("SECRET"));
}

#[test]
fn entry_projection_is_fresh_and_repeated_reads_are_independent() {
    let engine = literal_engine();
    let bundle = redacted_builder()
        .scan_and_build_core(&engine, vec![("one".into(), "SECRET".into())])
        .unwrap();
    let first = bundle.entry_at(0).unwrap();
    let second = bundle.entry_at(0).unwrap();

    assert_eq!(first.id(), second.id());
    assert_eq!(first.content(), second.content());
    assert!(bundle.entry_at_core(1).is_none());
}
