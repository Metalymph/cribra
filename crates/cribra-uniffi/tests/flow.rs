#![cfg(feature = "flow")]

#[path = "support/keyed_flow.rs"]
mod keyed;
#[path = "support/flow.rs"]
mod support;

use cribra_uniffi::{
    RedactOptions, TemplateOptions, TransformError, TransformResult, redact, template,
};
use serde_json::{Value, json};

fn outcome(result: Result<TransformResult, TransformError>) -> Value {
    match result {
        Ok(result) => json!({"content":result.content}),
        Err(TransformError::Rejected {
            code,
            span,
            other_span,
            source_len,
            detail,
        }) => {
            let bytes = |span: Option<cribra_uniffi::ByteSpan>| {
                span.map(|span| json!({"start":span.start,"end":span.end}))
            };
            json!({"error":{"code":format!("{code:?}"),"span":bytes(span),"otherSpan":bytes(other_span),"sourceLen":source_len,"detail":detail}})
        }
    }
}

#[test]
fn explicit_findings_match_core_outputs_and_structured_errors() {
    let cases = support::cases();
    assert_eq!(cases.len(), 23);
    for case in cases {
        let source = case["source"].as_str().unwrap().to_owned();
        let findings = support::findings(&case);
        let expected = support::core_outcomes(&case);
        let actual_redact = redact(
            source.clone(),
            findings.clone(),
            RedactOptions {
                replacement: case["replacement"].as_str().map(str::to_owned),
            },
        );
        let actual_template = template(
            source,
            findings,
            TemplateOptions {
                namespace: case["namespace"].as_str().map(str::to_owned),
                numbered: case["numbered"].as_bool().unwrap_or(false),
            },
        );
        assert_eq!(
            outcome(actual_redact),
            expected["redact"],
            "redact {}",
            case["name"]
        );
        assert_eq!(
            outcome(actual_template),
            expected["template"],
            "template {}",
            case["name"]
        );
    }
}

#[test]
fn no_findings_never_trigger_a_scan_and_unknown_ids_are_accepted() {
    let secret = "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";
    let scanner = cribra::Scanner::default();
    assert!(
        !scanner
            .scan([((), secret)])
            .single_report()
            .unwrap()
            .findings()
            .is_empty()
    );
    assert_eq!(
        redact(secret.into(), vec![], RedactOptions::default())
            .unwrap()
            .content,
        secret
    );
    assert_eq!(
        template(secret.into(), vec![], TemplateOptions::default())
            .unwrap()
            .content,
        secret
    );
    let cases = support::cases();
    let case = cases
        .iter()
        .find(|c| c["name"] == "explicit-unknown-rule")
        .unwrap();
    assert_eq!(
        redact(
            "ordinary".into(),
            support::findings(case),
            RedactOptions::default()
        )
        .unwrap()
        .content,
        "[REDACTED]"
    );
}

#[test]
fn scanner_findings_can_be_used_explicitly_and_results_outlive_inputs() {
    let result = {
        let source = String::from("GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789");
        let scanner = cribra_uniffi::Scanner::new();
        let report = scanner.scan(cribra_uniffi::ScanSource {
            id: cribra_uniffi::SourceId {
                value: "input".into(),
            },
            text: source.clone(),
        });
        redact(source, report.findings, RedactOptions::default()).unwrap()
    };
    assert_eq!(result.content, "GITHUB_TOKEN=[REDACTED]");
}

fn keyed_options(
    options: &Value,
) -> (
    cribra_uniffi::PseudonymizationOptions,
    cribra_uniffi::SynthesisOptions,
) {
    let key = keyed::test_key(options["key"].as_str().unwrap()).to_vec();
    (
        cribra_uniffi::PseudonymizationOptions {
            key: key.clone(),
            prefix: options["prefix"].as_str().map(str::to_owned),
            digest_bytes: options["digestBytes"].as_u64(),
        },
        cribra_uniffi::SynthesisOptions {
            key,
            marker: options["marker"].as_str().map(str::to_owned),
        },
    )
}

#[test]
fn keyed_transform_outputs_errors_and_determinism_match_core() {
    let cases = keyed::cases();
    let configs = keyed::options();
    assert_eq!((cases.len(), configs.len()), (27, 9));
    for case in cases {
        for options in &configs {
            let expected = keyed::core_outcomes(&case, options);
            let (pseudo, synthesis) = keyed_options(options);
            let source = case["source"].as_str().unwrap();
            let findings = support::findings(&case);
            let actual_pseudo = outcome(cribra_uniffi::pseudonymize(
                source.into(),
                findings.clone(),
                pseudo.clone(),
            ));
            let actual_synthesis = outcome(cribra_uniffi::synthesize(
                source.into(),
                findings.clone(),
                synthesis.clone(),
            ));
            assert_eq!(
                actual_pseudo, expected["pseudonymize"],
                "{} {}",
                case["name"], options["name"]
            );
            assert_eq!(
                actual_synthesis, expected["synthesize"],
                "{} {}",
                case["name"], options["name"]
            );
            assert_eq!(
                actual_pseudo,
                outcome(cribra_uniffi::pseudonymize(
                    source.into(),
                    findings.clone(),
                    pseudo
                ))
            );
            assert_eq!(
                actual_synthesis,
                outcome(cribra_uniffi::synthesize(
                    source.into(),
                    findings,
                    synthesis
                ))
            );
            let key = keyed::test_key(options["key"].as_str().unwrap());
            let public_key_text = std::str::from_utf8(&key).unwrap();
            assert!(!actual_pseudo.to_string().contains(public_key_text));
            assert!(!actual_synthesis.to_string().contains(public_key_text));
        }
    }
}

#[test]
fn wrong_length_keys_never_appear_in_errors_or_create_defaults() {
    use cribra_uniffi::{PseudonymizationOptions, SynthesisOptions, TransformErrorCode};
    for length in [0, 1, 31, 33, 64, 1024] {
        for (source, findings) in [
            (String::new(), vec![]),
            (
                String::from("abc"),
                support::findings(&support::cases()[12]),
            ),
        ] {
            let key = vec![0xa5; length];
            let pseudo = cribra_uniffi::pseudonymize(
                source.clone(),
                findings.clone(),
                PseudonymizationOptions {
                    key: key.clone(),
                    prefix: None,
                    digest_bytes: None,
                },
            )
            .unwrap_err();
            let synthesis =
                cribra_uniffi::synthesize(source, findings, SynthesisOptions { key, marker: None })
                    .unwrap_err();
            for error in [pseudo, synthesis] {
                let TransformError::Rejected {
                    code,
                    span,
                    other_span,
                    source_len,
                    detail,
                } = error;
                assert_eq!(code, TransformErrorCode::InvalidKeyLength);
                assert_eq!((span, other_span, source_len), (None, None, None));
                assert_eq!(
                    detail,
                    format!("transformation key must contain exactly 32 bytes; received {length}")
                );
                assert!(!detail.contains("165")); // no byte dump; only lengths above appear
            }
        }
    }
}

#[test]
fn key_identity_digest_clamping_and_synthesis_inputs_follow_core() {
    let cases = keyed::cases();
    let first = cases.iter().find(|c| c["name"] == "content-a").unwrap();
    let second = cases.iter().find(|c| c["name"] == "content-b").unwrap();
    let configs = keyed::options();
    let (pseudo_a, synth_a) = keyed_options(&configs[0]);
    let (pseudo_b, synth_b) = keyed_options(&configs[1]);
    let run_pseudo = |case: &Value, options| {
        cribra_uniffi::pseudonymize(
            case["source"].as_str().unwrap().into(),
            support::findings(case),
            options,
        )
        .unwrap()
        .content
    };
    let run_synth = |case: &Value, options| {
        cribra_uniffi::synthesize(
            case["source"].as_str().unwrap().into(),
            support::findings(case),
            options,
        )
        .unwrap()
        .content
    };
    assert_ne!(
        run_pseudo(first, pseudo_a.clone()),
        run_pseudo(first, pseudo_b)
    );
    assert_ne!(run_synth(first, synth_a.clone()), run_synth(first, synth_b));
    assert_ne!(
        run_pseudo(first, pseudo_a.clone()),
        run_pseudo(second, pseudo_a.clone())
    );
    // Synthesis uses key, ID and byte geometry, not original matched bytes.
    assert_eq!(
        run_synth(first, synth_a.clone()),
        run_synth(second, synth_a)
    );
    let equal = cases.iter().find(|c| c["name"] == "equal-values").unwrap();
    let output = run_pseudo(equal, pseudo_a.clone());
    let parts: Vec<_> = output.split(" x ").collect();
    assert_eq!(parts[0], parts[1]); // rule IDs and positions do not affect pseudonym identity
    for (requested, expected_bytes) in [
        (0, 8),
        (1, 8),
        (8, 8),
        (16, 16),
        (32, 32),
        (33, 32),
        (u64::MAX, 32),
    ] {
        let options = cribra_uniffi::PseudonymizationOptions {
            key: pseudo_a.key.clone(),
            prefix: Some("p:".into()),
            digest_bytes: Some(requested),
        };
        assert_eq!(run_pseudo(first, options).len(), 2 + expected_bytes * 2);
    }
}

#[test]
fn keyed_option_debug_redacts_keys_and_empty_findings_do_not_scan() {
    let configs = keyed::options();
    let (pseudo, synthesis) = keyed_options(&configs[0]);
    let public_key_text = std::str::from_utf8(&pseudo.key).unwrap();
    for debug in [format!("{pseudo:?}"), format!("{synthesis:?}")] {
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(public_key_text));
    }
    for source in ["", "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789"] {
        assert_eq!(
            cribra_uniffi::pseudonymize(source.into(), vec![], pseudo.clone())
                .unwrap()
                .content,
            source
        );
        assert_eq!(
            cribra_uniffi::synthesize(source.into(), vec![], synthesis.clone())
                .unwrap()
                .content,
            source
        );
    }
}
