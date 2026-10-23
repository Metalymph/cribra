use super::*;
use crate::{FindingSeverity, ScanEngineBuilder};

fn literal_engine() -> ScanEngine {
    let mut builder = ScanEngineBuilder::new(false);
    builder.add_literal("custom.secret", "SECRET", FindingSeverity::High);
    builder.build().expect("literal scanner should build")
}

#[test]
fn empty_batch_has_no_entries_and_zero_summary() {
    let engine = ScanEngine::new();
    let batch = engine.scan_batch_core(Vec::new());

    assert_eq!(batch.length(), 0);
    assert!(batch.is_empty());

    let summary = batch.summary();
    assert_eq!(summary.scanned_sources(), 0);
    assert_eq!(summary.scanned_bytes(), 0);
    assert_eq!(summary.reports_with_findings(), 0);
    assert_eq!(summary.reports_without_findings(), 0);
    assert_eq!(summary.total_findings(), 0);
    assert_eq!(summary.total_candidates(), 0);
    assert_eq!(summary.reports_with_candidates(), 0);
    assert!(!summary.has_candidates());
    assert!(summary.is_clean());
    assert!(!summary.has_critical());
    assert_eq!(summary.by_severity(FindingSeverity::Critical), 0);
}

#[test]
fn single_source_matches_scan_projection() {
    let engine = literal_engine();
    let source = "é SECRET";
    let whole = engine.scan(source);
    let batch = engine.scan_batch_core(vec![("notes.txt".into(), source.into())]);

    assert_eq!(batch.length(), 1);
    let entry = batch.entry_at(0).unwrap();
    let result = entry.result();

    assert_eq!(entry.id(), "notes.txt");
    assert_eq!(entry.source_bytes(), source.len());
    assert_eq!(result.source_bytes(), whole.source_bytes());
    assert_eq!(result.finding_count(), whole.finding_count());
    assert_eq!(result.candidate_count(), whole.candidate_count());
    assert_eq!(result.needs_review(), whole.needs_review());
    assert_eq!(result.has_critical(), whole.has_critical());
    assert_eq!(result.report, whole.report);

    let finding = entry.result().finding_at(0).unwrap();
    assert_eq!(finding.start(), "é ".len());
    assert_eq!(finding.end(), source.len());
}

#[test]
fn multiple_sources_preserve_input_order_and_byte_lengths() {
    let engine = literal_engine();
    let inputs = vec![
        ("z".to_owned(), "first SECRET".to_owned()),
        ("a".to_owned(), String::new()),
        ("m".to_owned(), "🦀 SECRET\nSECRET".to_owned()),
    ];
    let batch = engine.scan_batch_core(inputs.clone());

    assert_eq!(batch.length(), inputs.len());
    for (index, (id, source)) in inputs.iter().enumerate() {
        let entry = batch.entry_at(index).unwrap();
        assert_eq!(entry.id(), *id);
        assert_eq!(entry.source_bytes(), source.len());
        assert_eq!(entry.result().source_bytes(), source.len());
    }

    assert_eq!(batch.entry_at(0).unwrap().result().finding_count(), 1);
    assert_eq!(batch.entry_at(1).unwrap().result().finding_count(), 0);
    assert_eq!(batch.entry_at(2).unwrap().result().finding_count(), 2);
}

#[test]
fn duplicate_empty_and_unicode_ids_are_preserved_exactly() {
    let engine = literal_engine();
    let batch = engine.scan_batch_core(vec![
        ("same".into(), "SECRET one".into()),
        ("same".into(), "SECRET two".into()),
        (String::new(), "SECRET empty".into()),
        ("前🦀".into(), "é SECRET".into()),
    ]);

    assert_eq!(batch.entry_at(0).unwrap().id(), "same");
    assert_eq!(batch.entry_at(1).unwrap().id(), "same");
    assert_eq!(batch.entry_at(2).unwrap().id(), "");
    assert_eq!(batch.entry_at(3).unwrap().id(), "前🦀");
}

#[test]
fn summary_matches_direct_core_batch_summary() {
    let sources = [
        ("clean", "ordinary text"),
        ("finding", "SECRET"),
        ("review", "ABCD-EFGH-IJKL-MNOP"),
    ];
    let engine = literal_engine();
    let batch = engine.scan_batch_core(
        sources
            .iter()
            .map(|(id, source)| ((*id).to_owned(), (*source).to_owned()))
            .collect(),
    );
    let view = batch.summary();
    let core = engine
        .scanner
        .scan(sources.iter().map(|(id, source)| (*id, *source)))
        .summary();

    assert_eq!(view.scanned_sources(), core.scanned_sources());
    assert_eq!(view.scanned_bytes(), core.scanned_bytes());
    assert_eq!(view.reports_with_findings(), core.reports_with_findings());
    assert_eq!(
        view.reports_without_findings(),
        core.reports_without_findings()
    );
    assert_eq!(view.total_findings(), core.total_findings());
    assert_eq!(view.total_candidates(), core.total_candidates());
    assert_eq!(
        view.reports_with_candidates(),
        core.reports_with_candidates()
    );
    assert_eq!(view.has_candidates(), core.has_candidates());
    assert_eq!(view.critical(), core.critical());
    assert_eq!(view.high(), core.high());
    assert_eq!(view.medium(), core.medium());
    assert_eq!(view.low(), core.low());
    assert_eq!(view.info(), core.info());
    assert_eq!(view.is_clean(), core.is_clean());
    assert_eq!(view.has_critical(), core.has_critical());
    assert_eq!(
        view.by_severity(FindingSeverity::High),
        core.by_severity(Severity::High)
    );
}

#[test]
fn findings_and_candidates_keep_existing_report_semantics() {
    let engine = ScanEngine::new();
    let source = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\nABCD-EFGH-IJKL-MNOP";
    let batch = engine.scan_batch_core(vec![("mixed".into(), source.into())]);
    let entry = batch.entry_at(0).unwrap();
    let result = entry.result();

    assert_eq!(result.finding_count(), 1);
    assert_eq!(result.candidate_count(), 1);
    assert!(result.needs_review());
    assert_eq!(entry.source_bytes(), source.len());
    assert_eq!(
        result.finding_at(0).unwrap().rule_id(),
        "github.classic-pat"
    );
    assert_eq!(
        result.candidate_at(0).unwrap().kind(),
        crate::CandidateKind::RecoveryLikeCode
    );
}

#[test]
fn empty_and_unicode_source_contents_are_not_retained_or_altered() {
    let engine = literal_engine();
    let batch = engine.scan_batch_core(vec![
        ("empty".into(), String::new()),
        ("unicode".into(), "🦀é前 SECRET".into()),
    ]);

    assert_eq!(batch.entry_at(0).unwrap().source_bytes(), 0);
    assert_eq!(
        batch.entry_at(1).unwrap().source_bytes(),
        "🦀é前 SECRET".len()
    );
    assert_eq!(
        batch
            .entry_at(1)
            .unwrap()
            .result()
            .finding_at(0)
            .unwrap()
            .start(),
        "🦀é前 ".len()
    );
    assert_eq!(batch.summary().scanned_bytes(), "🦀é前 SECRET".len());
}

#[test]
fn custom_rules_and_opt_in_catalogs_work_for_batches() {
    let mut builder = ScanEngineBuilder::new(false);
    builder.add_financial_builtins();
    builder.add_personal_builtins();
    builder.add_literal("custom.secret", "SECRET", FindingSeverity::Critical);
    let engine = builder.build().expect("configured scanner should build");

    let batch = engine.scan_batch_core(vec![
        ("financial".into(), "card_number=1234567890123452".into()),
        ("personal".into(), "ssn=123-45-6789".into()),
        ("custom".into(), "SECRET".into()),
    ]);

    assert_eq!(batch.entry_at(0).unwrap().result().finding_count(), 1);
    assert_eq!(batch.entry_at(1).unwrap().result().finding_count(), 1);
    let custom = batch.entry_at(2).unwrap().result();
    assert_eq!(custom.finding_count(), 1);
    assert_eq!(custom.finding_at(0).unwrap().rule_id(), "custom.secret");
    assert_eq!(batch.summary().critical(), 1);
}

#[test]
fn batches_and_entries_outlive_the_engine_and_each_other() {
    let engine = literal_engine();
    let batch = engine.scan_batch_core(vec![
        ("one".into(), "SECRET".into()),
        ("two".into(), "ordinary".into()),
    ]);
    let entry = batch.entry_at(0).unwrap();
    let result = entry.result();
    drop(engine);
    drop(batch);

    assert_eq!(entry.id(), "one");
    assert_eq!(entry.source_bytes(), "SECRET".len());
    assert_eq!(result.finding_count(), 1);
    assert_eq!(entry.result().finding_count(), 1);
    assert_eq!(entry.result().source_bytes(), result.source_bytes());
}

#[test]
fn repeated_batches_and_derived_results_are_independent() {
    let engine = literal_engine();
    let first = engine.scan_batch_core(vec![("id".into(), "SECRET".into())]);
    let second = engine.scan_batch_core(vec![("id".into(), "SECRET".into())]);
    let first_entry = first.entry_at(0).unwrap();
    let second_entry = second.entry_at(0).unwrap();
    let first_result = first_entry.result();
    let second_result = second_entry.result();

    drop(first);
    assert_eq!(second_result.finding_count(), 1);
    assert_eq!(second_entry.id(), "id");
    drop(second);
    assert_eq!(first_result.finding_count(), 1);
    assert_eq!(first_entry.source_bytes(), "SECRET".len());
}
