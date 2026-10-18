//! Direct Core oracle for revision, edit and terminal-resource behavior.

#[path = "../../tests/support/incremental.rs"]
mod support;

use cribra::{IncrementalLimits, IncrementalSession, Rule, Scanner, Severity};
use serde_json::{Value, json};

fn view(session: &IncrementalSession, id: &str) -> Value {
    json!({"revision":session.revision(),"state":if session.is_active() {"Active"} else {"Failed"},
        "report": super::report_snapshot(id, session.source().len(), session.report())})
}

pub fn data() -> Value {
    let fixture = support::fixture();
    let id = fixture["id"].as_str().unwrap();
    let case = super::configuration::cases()
        .into_iter()
        .find(|c| c.name == "custom")
        .unwrap();
    let scanner = super::configuration::core_build(&case.config).unwrap();
    let mut session = scanner
        .incremental_session(fixture["source"].as_str().unwrap().into())
        .unwrap();
    let mut trace = vec![json!({"snapshot":view(&session,id)})];
    for edit in fixture["edits"].as_array().unwrap() {
        let result = session.apply_edit(
            edit["revision"].as_u64().unwrap(),
            edit["start"].as_u64().unwrap() as usize..edit["end"].as_u64().unwrap() as usize,
            edit["replacement"].as_str().unwrap(),
        );
        let (revision, error) = match result {
            Ok(revision) => (Some(revision), None),
            Err(error) => (None, Some(support::core_error(error, !session.is_active()))),
        };
        assert_eq!(
            session.report(),
            scanner
                .scan([((), session.source())])
                .single_report()
                .unwrap()
        );
        trace.push(json!({"newRevision":revision,"error":error,"snapshot":view(&session,id)}));
    }
    let mut resources = Vec::new();
    for (name, limits, initial, replacement) in [
        (
            "candidates",
            IncrementalLimits::default().with_max_candidates(Some(1)),
            "x",
            "xx",
        ),
        (
            "cache",
            IncrementalLimits::default().with_max_retained_cache_records(Some(1)),
            "x",
            "",
        ),
        (
            "results",
            IncrementalLimits::default().with_max_materialized_results(Some(1)),
            "x",
            "x",
        ),
        (
            "storage",
            IncrementalLimits::default().with_max_transaction_storage_bytes(Some(0)),
            "",
            "a",
        ),
    ] {
        let mut builder = Scanner::builder();
        if !initial.is_empty() {
            builder = builder.rule(Rule::literal("literal", "x", Severity::High));
        }
        let scanner = builder.build().unwrap();
        let mut session = scanner
            .incremental_session_with_limits(initial.into(), limits)
            .unwrap();
        if initial.is_empty() {
            session.apply_edit(0, 0..0, "").unwrap();
        }
        let before = view(&session, id);
        let error = session
            .apply_edit(session.revision(), 0..0, replacement)
            .unwrap_err();
        let error = support::core_error(error, !session.is_active());
        let after = view(&session, id);
        let later = session.apply_edit(u64::MAX, 999..0, "").unwrap_err();
        resources.push(
            json!({"name":name,"before":before,"after":after,"error":error,
            "laterError":support::core_error(later,!session.is_active())}),
        );
    }
    let scanner = Scanner::builder()
        .rule(Rule::literal("literal", "x", Severity::High))
        .build()
        .unwrap();
    let construction: Vec<Value> = [
        (
            "candidates",
            IncrementalLimits::default().with_max_candidates(Some(0)),
        ),
        (
            "cache",
            IncrementalLimits::default().with_max_retained_cache_records(Some(0)),
        ),
        (
            "results",
            IncrementalLimits::default().with_max_materialized_results(Some(0)),
        ),
        (
            "storage",
            IncrementalLimits::default().with_max_transaction_storage_bytes(Some(0)),
        ),
    ]
    .into_iter()
    .map(|(name, limits)| {
        json!({"name":name,"error":support::core_error(
        scanner.incremental_session_with_limits("x".into(),limits).unwrap_err(),true)})
    })
    .collect();
    json!({"trace":trace,"resources":resources,"construction":construction})
}
