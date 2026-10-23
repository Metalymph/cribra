#![cfg(feature = "core")]

use cribra_uniffi::{
    CandidateEvidence, DetectionMode, Explanation, Finding, Remediation, ScanSource, Scanner,
    SensitiveCandidateKind, SourceId,
};

fn source(text: &str) -> ScanSource {
    ScanSource {
        id: SourceId {
            value: "metadata".into(),
        },
        text: text.into(),
    }
}

#[test]
fn finding_explanation_delegates_to_core_authority_and_fails_closed() {
    let rule = || {
        cribra::Rule::literal("authority", "SECRET", cribra::Severity::High)
            .with_remediation(cribra::Remediation::RotateCredential)
    };
    let scanner = cribra::Scanner::builder().rule(rule()).build().unwrap();
    let results = scanner.scan([((), "SECRET")]);
    let finding = &results.single_report().unwrap().findings()[0];
    let compatible = cribra::Scanner::builder().rule(rule()).build().unwrap();
    for authority in [&scanner, &compatible] {
        assert!(finding.explanation(authority).is_some());
        assert_eq!(
            Finding::from_core(finding, authority).explanation,
            Some(Explanation::Classified {
                mode: DetectionMode::MatcherOnly
            })
        );
    }
    // Real Core findings and real scanners only: no serde reconstruction or
    // parallel validation logic in the adapter or the authority test.
    for authority in [
        cribra::Scanner::builder().build().unwrap(),
        cribra::Scanner::builder()
            .rule(
                cribra::Rule::literal("other", "SECRET", cribra::Severity::High)
                    .with_remediation(cribra::Remediation::RotateCredential),
            )
            .build()
            .unwrap(),
        cribra::Scanner::builder()
            .rule(
                cribra::Rule::literal("authority", "SECRET", cribra::Severity::Low)
                    .with_remediation(cribra::Remediation::RotateCredential),
            )
            .build()
            .unwrap(),
        cribra::Scanner::builder()
            .rule(cribra::Rule::literal(
                "authority",
                "SECRET",
                cribra::Severity::High,
            ))
            .build()
            .unwrap(),
        cribra::Scanner::builder()
            .rule(
                cribra::Rule::literal("authority", "SECRET", cribra::Severity::High)
                    .with_remediation(cribra::Remediation::RotatePassword),
            )
            .build()
            .unwrap(),
    ] {
        assert_eq!(finding.explanation(&authority), None);
        let projected = Finding::from_core(finding, &authority);
        assert_eq!(projected.explanation, None);
        assert_eq!(projected.remediation, Some(Remediation::RotateCredential));
    }
    // The legacy scanner-free conversion must never infer authority.
    assert_eq!(Finding::from(finding).explanation, None);
}

#[test]
fn every_remediation_is_preserved_without_flow_or_core_serde() {
    for action in [
        cribra::Remediation::RevokeAndRotateCredential,
        cribra::Remediation::RotateCredential,
        cribra::Remediation::RotatePassword,
        cribra::Remediation::ReplacePrivateKey,
        cribra::Remediation::RemoveSensitiveValue,
        cribra::Remediation::ReviewSensitiveHash,
        cribra::Remediation::ReviewPasswordVerifier,
    ] {
        let scanner = cribra::Scanner::builder()
            .rule(
                cribra::Rule::literal("custom", "SECRET", cribra::Severity::High)
                    .with_remediation(action),
            )
            .build()
            .unwrap();
        let results = scanner.scan([((), "SECRET")]);
        let finding = &results.single_report().unwrap().findings()[0];
        assert_eq!(
            cribra::Remediation::from(Finding::from_core(finding, &scanner).remediation.unwrap()),
            action
        );
    }
}

#[test]
fn reports_preserve_candidate_metadata_utf8_order_and_all_detection_modes() {
    let scanner = Scanner::new();
    let text = "前\r\né🙂e\u{301} ABCD-EFGH-IJKL-MNOP\npassword=CorrectHorseBatteryStaple\nGITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\nQRST-UVWX-YZ12-3456";
    let core = cribra::Scanner::default();
    let results = core.scan([((), text)]);
    let expected = results.single_report().unwrap();
    let report = scanner.scan(source(text));
    assert_eq!(report.candidate_count, 2);
    assert_eq!(report.candidates.len(), expected.candidate_len());
    for (actual, candidate) in report.candidates.iter().zip(expected.candidates()) {
        assert_eq!(actual.kind, SensitiveCandidateKind::RecoveryLikeCode);
        assert_eq!(actual.location, candidate.location().into());
        assert_eq!(actual.evidence, CandidateEvidence::Structural);
        assert_eq!(
            actual.explanation,
            Some(Explanation::Ambiguous {
                evidence: CandidateEvidence::Structural
            })
        );
    }
    assert_eq!(report.candidates[0].location.start, 15);
    assert_eq!(report.candidates[0].location.column, 6);
    assert!(report.findings.iter().any(|f| f.explanation
        == Some(Explanation::Classified {
            mode: DetectionMode::Contextual
        })));
    assert!(report.findings.iter().any(|f| f.explanation
        == Some(Explanation::Classified {
            mode: DetectionMode::Deterministic
        })));
    assert_eq!(
        report.findings,
        expected
            .findings()
            .iter()
            .map(|f| Finding::from_core(f, &core))
            .collect::<Vec<_>>()
    );

    let incremental = scanner.incremental(source(text)).unwrap();
    assert_eq!(incremental.snapshot().unwrap().report, report);
    // Default built-ins retain Core's unsupported-stream admission behavior.
    assert!(scanner.stream(source(text).id).is_err());
    let stream_scanner = Scanner::configured(cribra_uniffi::ScannerConfig::CustomOnly {
        rules: vec![cribra_uniffi::RuleSpec {
            id: cribra_uniffi::RuleId {
                value: "literal".into(),
            },
            kind: cribra_uniffi::RuleKind::Literal,
            value: "CorrectHorseBatteryStaple".into(),
            severity: cribra_uniffi::Severity::High,
            remediation: Some(Remediation::RotatePassword),
        }],
    })
    .unwrap();
    let expected_stream = stream_scanner.scan(source(text));
    let stream = stream_scanner.stream(source(text).id).unwrap();
    drop(stream_scanner);
    // Split inside multibyte scalars as well as both candidate spans.
    for byte in text.as_bytes() {
        stream.push(vec![*byte]).unwrap();
    }
    drop(scanner);
    assert_eq!(stream.finish().unwrap(), expected_stream);
    assert_eq!(incremental.report().unwrap(), report);
    let view = format!("{report:?}");
    for secret in [
        "ABCD-EFGH-IJKL-MNOP",
        "CorrectHorseBatteryStaple",
        "ghp_AbCdEf0123456789_AbCdEf0123456789",
    ] {
        assert!(!view.contains(secret));
    }
}
