#![cfg(feature = "flow")]
#[path = "support/flow.rs"]
#[allow(dead_code)]
mod flow;
#[path = "support/share.rs"]
mod support;
use cribra_uniffi::*;
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

fn scanner(builtins: bool) -> Arc<Scanner> {
    if builtins {
        Scanner::new()
    } else {
        Scanner::configured(ScannerConfig::CustomOnly {
            rules: vec![RuleSpec {
                id: RuleId {
                    value: "secret".into(),
                },
                kind: RuleKind::Literal,
                value: "SECRET".into(),
                severity: Severity::High,
                remediation: None,
            }],
        })
        .unwrap()
    }
}
fn options(mode: usize) -> ShareBundleOptions {
    ShareBundleOptions {
        mode: Some(match mode {
            0 => ShareMode::Redact,
            1 => ShareMode::Template,
            2 => ShareMode::Pseudonymize {
                options: PseudonymizationOptions {
                    key: support::KEY.to_vec(),
                    prefix: Some("p:".into()),
                    digest_bytes: Some(8),
                },
            },
            3 => ShareMode::Synthesize {
                options: SynthesisOptions {
                    key: support::KEY.to_vec(),
                    marker: Some("demo".into()),
                },
            },
            _ => unreachable!(),
        }),
    }
}
fn outcome(result: Result<ShareBundle, ShareError>, before: SystemTime) -> Value {
    match result {
        Ok(b) => {
            let t = &b.manifest.generated_at;
            assert!(t.nanoseconds < 1_000_000_000);
            let actual = UNIX_EPOCH + std::time::Duration::new(t.seconds, t.nanoseconds);
            assert!(before <= actual && actual <= SystemTime::now());
            let mode = match b.manifest.mode {
                ShareModeKind::Redact => "redact",
                ShareModeKind::Template => "template",
                ShareModeKind::Pseudonymize => "pseudonymize",
                ShareModeKind::Synthesize => "synthesize",
            };
            let s = b.manifest.summary;
            json!({"sources":b.sources.iter().map(|s|json!({"id":s.source_id.value,"content":s.content})).collect::<Vec<_>>(),"mode":mode,
                "summary":{"scanned_sources":s.scanned_sources,"scanned_bytes":s.scanned_bytes,"reports_with_findings":s.reports_with_findings,"reports_without_findings":s.reports_without_findings,"total_findings":s.total_findings,"total_candidates":s.total_candidates,"reports_with_candidates":s.reports_with_candidates,"critical":s.critical,"high":s.high,"medium":s.medium,"low":s.low,"info":s.info}})
        }
        Err(ShareError::Rejected {
            code,
            expected,
            actual,
            index,
            detail,
        }) => {
            json!({"error":{"kind":"bundle","code":format!("{code:?}"),"expected":expected,"actual":actual,"index":index,"detail":detail}})
        }
        Err(ShareError::Transform {
            error:
                TransformError::Rejected {
                    code,
                    span,
                    other_span,
                    source_len,
                    detail,
                },
        }) => {
            let bytes = |s: Option<ByteSpan>| s.map(|s| json!({"start":s.start,"end":s.end}));
            json!({"error":{"kind":"transform","code":format!("{code:?}"),"span":bytes(span),"otherSpan":bytes(other_span),"sourceLen":source_len,"detail":detail}})
        }
    }
}
fn inputs(case: &Value) -> Vec<ScanSource> {
    case["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| ScanSource {
            id: SourceId {
                value: s["id"].as_str().unwrap().into(),
            },
            text: s["text"].as_str().unwrap().into(),
        })
        .collect()
}
#[test]
fn all_modes_and_both_core_paths_preserve_order_ids_candidates_and_manifest() {
    let mut candidates = 0;
    for case in support::batches() {
        let sources = inputs(&case);
        let scanner = scanner(case["builtins"].as_bool().unwrap_or(false));
        let reports = scanner.scan_for_share(sources.clone()).unwrap();
        assert_eq!(reports.len(), sources.len());
        candidates += reports.iter().map(|r| r.candidates.len()).sum::<usize>();
        for mode in 0..4 {
            for atomic in [false, true] {
                let before = SystemTime::now();
                let result = if atomic {
                    scan_and_build(scanner.clone(), sources.clone(), options(mode))
                } else {
                    build(
                        reports.clone(),
                        sources.iter().map(|s| s.text.clone()).collect(),
                        options(mode),
                    )
                };
                let actual = outcome(result, before);
                assert_eq!(
                    actual,
                    support::core_batch(&case, mode, atomic),
                    "{} mode {mode}",
                    case["name"]
                );
                assert!(
                    !actual
                        .to_string()
                        .contains(std::str::from_utf8(support::KEY).unwrap())
                );
            }
        }
    }
    assert!(candidates > 0, "candidate counters must be exercised");
}
#[test]
fn explicit_reports_match_core_for_all_modes_spans_boundaries_overlap_and_no_scan() {
    for case in flow::cases() {
        let source = case["source"].as_str().unwrap().to_owned();
        let report = ShareReport {
            source_id: SourceId {
                value: "明示😀".into(),
            },
            source_bytes: source.len() as u64,
            findings: flow::findings(&case),
            candidates: vec![],
        };
        for mode in 0..4 {
            let before = SystemTime::now();
            let actual = outcome(
                build(vec![report.clone()], vec![source.clone()], options(mode)),
                before,
            );
            assert_eq!(
                actual,
                support::core_explicit(&case, mode),
                "{} mode {mode}",
                case["name"]
            );
            assert!(
                !actual
                    .to_string()
                    .contains(std::str::from_utf8(support::KEY).unwrap())
            );
        }
    }
}
#[test]
fn bundle_errors_have_core_numeric_details_and_same_length_changes_are_accepted() {
    let scanner = scanner(false);
    let reports = scanner
        .scan_for_share(vec![ScanSource {
            id: SourceId { value: "a".into() },
            text: "SECRET".into(),
        }])
        .unwrap();
    let core = support::scanner(false);
    let core_reports = core.scan([("a".to_owned(), "SECRET")]);
    for source_list in [vec![], vec!["SECRET", "extra"], vec!["short"]] {
        let core_result = cribra::transform::ShareBundle::builder()
            .mode(support::mode(0))
            .build(&core_reports, source_list.clone());
        let before = SystemTime::now();
        assert_eq!(
            outcome(
                build(
                    reports.clone(),
                    source_list.iter().map(|s| s.to_string()).collect(),
                    options(0)
                ),
                before
            ),
            support::outcome(core_result)
        );
    }
    let before = SystemTime::now();
    assert_eq!(
        outcome(
            build(reports.clone(), vec!["PUBLIC".into()], options(0)),
            before
        ),
        support::outcome(
            cribra::transform::ShareBundle::builder()
                .mode(support::mode(0))
                .build(&core_reports, ["PUBLIC"])
        )
    );
    // Count validation precedes length checks, and missing-mode precedes both in Core.
    let missing = ShareBundleOptions { mode: None };
    let before = SystemTime::now();
    assert_eq!(
        outcome(build(reports, vec![], missing.clone()), before),
        support::outcome(cribra::transform::ShareBundle::builder().build(&core_reports, []))
    );
    let before = SystemTime::now();
    assert_eq!(
        outcome(scan_and_build(scanner, vec![], missing), before),
        support::outcome(
            cribra::transform::ShareBundle::builder()
                .scan_and_build(&core, Vec::<(String, &str)>::new())
        )
    );
}
#[test]
fn explicit_keys_are_required_and_never_echoed_by_diagnostics() {
    for length in [0, 1, 31, 33, 64] {
        for mode in 2..4 {
            let mut opts = options(mode);
            match opts.mode.as_mut().unwrap() {
                ShareMode::Pseudonymize { options } => options.key = vec![b'Z'; length],
                ShareMode::Synthesize { options } => options.key = vec![b'Z'; length],
                _ => unreachable!(),
            }
            assert!(!format!("{opts:?}").contains("90, 90"));
            for atomic in [false, true] {
                let error = if atomic {
                    scan_and_build(scanner(false), vec![], opts.clone()).unwrap_err()
                } else {
                    build(vec![], vec![], opts.clone()).unwrap_err()
                };
                assert!(matches!(
                    &error,
                    ShareError::Transform {
                        error: TransformError::Rejected {
                            code: TransformErrorCode::InvalidKeyLength,
                            ..
                        }
                    }
                ));
                assert_eq!(
                    error.to_string(),
                    format!("transformation key must contain exactly 32 bytes; received {length}")
                );
            }
        }
    }
}
