//! Direct-Core committed snapshots for generated WASM incremental qualification.
mod oracle_support;
use serde_json::{Value, json};

fn error_code(error: cribra::IncrementalError) -> String {
    // Core's own variant name, independent of WASM enums and diagnostic mapping.
    format!("{error:?}")
        .split([' ', '{'])
        .next()
        .unwrap()
        .to_owned()
}

fn state(
    scanner: &cribra::Scanner,
    session: &cribra::IncrementalSession,
    error: Option<cribra::IncrementalError>,
) -> Value {
    let whole = scanner.scan([((), session.source())]);
    assert_eq!(whole.single_report().unwrap(), session.report());
    json!({
        "revision": session.revision().to_string(), "source": session.source(), "active":session.is_active(),
        "report":oracle_support::report(scanner,session.report(),session.source().len()),
        "error":error.map(|e| json!({"code":error_code(e), "terminal":!session.is_active()}))
    })
}

fn main() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("../tests/runtime/incremental-cases.json")).unwrap();
    let mut output = Vec::new();
    for case in cases {
        let scanner = oracle_support::scanner(&case);
        let mut limits = cribra::IncrementalLimits::default();
        let limit = |name: &str| {
            case["limits"][name]
                .as_u64()
                .map(|n| usize::try_from(n).unwrap())
        };
        limits = limits
            .with_max_candidates(limit("maxCandidates"))
            .with_max_retained_cache_records(limit("maxRetainedCacheRecords"))
            .with_max_materialized_results(limit("maxMaterializedResults"))
            .with_max_transaction_storage_bytes(limit("maxTransactionStorageBytes"));
        let session = scanner
            .incremental_session_with_limits(case["source"].as_str().unwrap().into(), limits);
        let mut session = match session {
            Ok(session) => session,
            Err(error) => {
                output.push(
                    json!({"name":case["name"],"constructionError":error_code(error),"states":[]}),
                );
                continue;
            }
        };
        let mut states = vec![state(&scanner, &session, None)];
        for edit in case["edits"].as_array().unwrap() {
            let revision = edit["revision"].as_str().unwrap().parse().unwrap();
            let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
            let end = if edit["end"] == "end" {
                session.source().len()
            } else {
                usize::try_from(edit["end"].as_u64().unwrap()).unwrap()
            };
            let error = session
                .apply_edit(revision, start..end, edit["replacement"].as_str().unwrap())
                .err();
            states.push(state(&scanner, &session, error));
        }
        output.push(json!({"name":case["name"],"constructionError":null,"states":states}));
    }
    println!("{}", json!(output));
}
