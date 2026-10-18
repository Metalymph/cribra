#![cfg(feature = "core")]

#[path = "support/configuration.rs"]
mod configuration;

use cribra_uniffi::{
    RuleId, RuleKind, RuleSpec, ScanSource, Scanner, ScannerConfig, SourceId, StreamError,
    StreamErrorCode, StreamLimits, StreamState,
};

fn id() -> SourceId {
    SourceId {
        value: "stream/文書é🙂".into(),
    }
}

fn literal_config(value: &str) -> ScannerConfig {
    ScannerConfig::CustomOnly {
        rules: vec![RuleSpec {
            id: RuleId {
                value: "literal".into(),
            },
            kind: RuleKind::Literal,
            value: value.into(),
            severity: cribra_uniffi::Severity::High,
            remediation: None,
        }],
    }
}

fn assert_error(error: StreamError, expected: cribra::StreamError) {
    assert_eq!(error, StreamError::from(expected));
}

#[test]
fn every_byte_split_and_chunk_size_match_whole_source_for_admitted_rules() {
    let case = configuration::cases()
        .into_iter()
        .find(|c| c.name == "custom")
        .unwrap();
    let core = configuration::core_build(&case.config).unwrap();
    let scanner = Scanner::configured(case.config).unwrap();
    let text = format!("{}ABCD-EFGH-IJKL-MNOP", case.source);
    let source = ScanSource { id: id(), text };
    let expected = scanner.scan(source.clone());
    assert_eq!(expected.findings.len(), 4);
    assert_eq!(expected.candidate_count, 1);
    let bytes = source.text.as_bytes();
    for split in 0..=bytes.len() {
        let session = scanner.stream(id()).unwrap();
        let mut direct = core.stream().unwrap();
        assert_eq!(session.state(), StreamState::Active);
        for chunk in [&bytes[..split], &[][..], &bytes[split..]] {
            session.push(chunk.to_vec()).unwrap();
            direct.push(chunk).unwrap();
        }
        let direct_report = direct.finish().unwrap();
        assert_eq!(
            &direct_report,
            core.scan([((), source.text.as_str())])
                .single_report()
                .unwrap()
        );
        assert_eq!(session.finish().unwrap(), expected, "split {split}");
        assert_eq!(session.state(), StreamState::Finished);
    }
    for size in [1, 2, 3, 7, 31, bytes.len()] {
        let session = scanner.stream(id()).unwrap();
        for chunk in bytes.chunks(size) {
            session.push(chunk.to_vec()).unwrap();
        }
        assert_eq!(session.finish().unwrap(), expected, "chunk size {size}");
    }
}

#[test]
fn empty_input_consuming_finish_and_scanner_lifetime() {
    let scanner = Scanner::configured(literal_config("SECRET")).unwrap();
    let session = scanner.stream(id()).unwrap();
    let expected = scanner.scan(ScanSource {
        id: id(),
        text: String::new(),
    });
    drop(scanner);
    session.push(vec![]).unwrap();
    assert_eq!(session.finish().unwrap(), expected);
    assert_error(
        session.finish().unwrap_err(),
        cribra::StreamError::NotActive,
    );
    assert_error(
        session.push(vec![]).unwrap_err(),
        cribra::StreamError::NotActive,
    );
    assert_eq!(session.state(), StreamState::Finished);
}

#[test]
fn invalid_and_incomplete_utf8_are_terminal_without_partial_reports() {
    let config = literal_config("SECRET");
    let core = configuration::core_build(&config).unwrap();
    let scanner = Scanner::configured(config).unwrap();
    for bytes in [
        vec![0xff],
        vec![0xc0, 0xaf],
        vec![0xed, 0xa0, 0x80],
        vec![0xe2, 0x28, 0xa1],
    ] {
        let session = scanner.stream(id()).unwrap();
        let mut direct = core.stream().unwrap();
        session.push(b"SECRET ".to_vec()).unwrap();
        direct.push(b"SECRET ").unwrap();
        let expected = direct.push(&bytes).unwrap_err();
        assert_error(session.push(bytes).unwrap_err(), expected);
        assert_eq!(session.state(), StreamState::Failed);
        assert_error(session.finish().unwrap_err(), direct.finish().unwrap_err());
        assert_error(
            session.push(vec![]).unwrap_err(),
            cribra::StreamError::NotActive,
        );
        assert_eq!(session.state(), StreamState::Failed);
    }
    for bytes in [vec![0xc3], vec![0xf0, 0x9f], vec![0xe2, 0x82]] {
        let session = scanner.stream(id()).unwrap();
        let mut direct = core.stream().unwrap();
        session.push(bytes.clone()).unwrap();
        direct.push(&bytes).unwrap();
        assert_eq!(session.state(), StreamState::Active);
        assert_error(session.finish().unwrap_err(), direct.finish().unwrap_err());
        assert_eq!(session.state(), StreamState::Failed);
        assert_error(
            session.finish().unwrap_err(),
            cribra::StreamError::NotActive,
        );
    }
}

#[test]
fn resource_errors_and_exact_storage_admission_match_core() {
    let config = literal_config("SECRET");
    let core = configuration::core_build(&config).unwrap();
    let scanner = Scanner::configured(config).unwrap();
    let denied = core
        .stream_with_limits(cribra::StreamLimits::default().with_max_stream_storage_bytes(Some(0)))
        .unwrap_err();
    let cribra::StreamError::StorageLimitExceeded { required, .. } = denied else {
        panic!("{denied}")
    };
    for allowance in [0, required - 1] {
        let expected = core
            .stream_with_limits(
                cribra::StreamLimits::default().with_max_stream_storage_bytes(Some(allowance)),
            )
            .unwrap_err();
        let error = scanner
            .stream_with_limits(
                id(),
                StreamLimits {
                    max_stream_storage_bytes: Some(allowance as u64),
                    ..StreamLimits::default()
                },
            )
            .err()
            .unwrap();
        assert_error(error, expected);
    }
    let exact = scanner
        .stream_with_limits(
            id(),
            StreamLimits {
                max_stream_storage_bytes: Some(required as u64),
                ..StreamLimits::default()
            },
        )
        .unwrap();
    exact.push(b"SECRET".to_vec()).unwrap();
    assert_eq!(exact.finish().unwrap().findings.len(), 1);

    for limits in [
        StreamLimits {
            max_candidates: Some(1),
            ..StreamLimits::default()
        },
        StreamLimits {
            max_retained_results: Some(1),
            ..StreamLimits::default()
        },
    ] {
        let core_limits = cribra::StreamLimits::default()
            .with_max_candidates(limits.max_candidates.map(|n| n as usize))
            .with_max_retained_results(limits.max_retained_results.map(|n| n as usize));
        let mut direct = core.stream_with_limits(core_limits).unwrap();
        let session = scanner.stream_with_limits(id(), limits).unwrap();
        session.push(b"SECRET ".to_vec()).unwrap();
        direct.push(b"SECRET ").unwrap();
        let expected = direct.push(b"SECRET ").unwrap_err();
        assert_error(session.push(b"SECRET ".to_vec()).unwrap_err(), expected);
        assert_eq!(session.state(), StreamState::Failed);
        assert_error(session.finish().unwrap_err(), direct.finish().unwrap_err());
    }
}

#[test]
fn eof_resource_limits_and_pre_suppression_counts_remain_core_owned() {
    for (config, bytes, limits) in [
        (
            ScannerConfig::CustomOnly { rules: vec![] },
            b"ABCD-EFGH-IJKL-MNOP".as_slice(),
            StreamLimits {
                max_retained_results: Some(0),
                ..StreamLimits::default()
            },
        ),
        (
            ScannerConfig::CustomOnly {
                rules: vec![RuleSpec {
                    id: RuleId {
                        value: "eof".into(),
                    },
                    kind: RuleKind::Pattern,
                    value: r"abc\z".into(),
                    severity: cribra_uniffi::Severity::High,
                    remediation: None,
                }],
            },
            b"abc".as_slice(),
            StreamLimits {
                max_candidates: Some(0),
                ..StreamLimits::default()
            },
        ),
    ] {
        let core = configuration::core_build(&config).unwrap();
        let scanner = Scanner::configured(config).unwrap();
        let mut direct = core
            .stream_with_limits(
                cribra::StreamLimits::default()
                    .with_max_candidates(limits.max_candidates.map(|n| n as usize))
                    .with_max_retained_results(limits.max_retained_results.map(|n| n as usize)),
            )
            .unwrap();
        let session = scanner.stream_with_limits(id(), limits).unwrap();
        direct.push(bytes).unwrap();
        session.push(bytes.to_vec()).unwrap();
        assert_error(session.finish().unwrap_err(), direct.finish().unwrap_err());
        assert_eq!(session.state(), StreamState::Failed);
    }
    let config = literal_config("ABCD-EFGH-IJKL-MNOP");
    let core = configuration::core_build(&config).unwrap();
    let scanner = Scanner::configured(config).unwrap();
    let mut direct = core
        .stream_with_limits(cribra::StreamLimits::default().with_max_retained_results(Some(1)))
        .unwrap();
    let session = scanner
        .stream_with_limits(
            id(),
            StreamLimits {
                max_retained_results: Some(1),
                ..StreamLimits::default()
            },
        )
        .unwrap();
    let bytes = b"ABCD-EFGH-IJKL-MNOP ";
    assert_error(
        session.push(bytes.to_vec()).unwrap_err(),
        direct.push(bytes).unwrap_err(),
    );
    assert_eq!(session.state(), StreamState::Failed);
}

#[test]
fn unsupported_rulesets_are_rejected_without_fallback() {
    for config in [
        ScannerConfig::BuiltInOnly,
        ScannerConfig::CustomOnly {
            rules: vec![RuleSpec {
                id: RuleId {
                    value: "unbounded".into(),
                },
                kind: RuleKind::Pattern,
                value: "a+".into(),
                severity: cribra_uniffi::Severity::High,
                remediation: None,
            }],
        },
    ] {
        let core = configuration::core_build(&config).unwrap();
        let scanner = Scanner::configured(config).unwrap();
        assert_error(
            scanner.stream(id()).err().unwrap(),
            core.stream().unwrap_err(),
        );
        // Streaming denial does not poison or alter the reusable Scanner.
        let text = "aaa GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789";
        assert!(
            !scanner
                .scan(ScanSource {
                    id: id(),
                    text: text.into()
                })
                .findings
                .is_empty()
        );
    }
}

#[test]
fn concurrent_finish_consumes_core_only_once() {
    let scanner = Scanner::configured(literal_config("SECRET")).unwrap();
    let session = scanner.stream(id()).unwrap();
    session.push(b"SECRET".to_vec()).unwrap();
    let other = session.clone();
    let thread = std::thread::spawn(move || other.finish());
    let first = session.finish();
    let second = thread.join().unwrap();
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let error = first.err().or_else(|| second.err()).unwrap();
    assert_error(error, cribra::StreamError::NotActive);
    assert_eq!(session.state(), StreamState::Finished);
}

#[test]
fn rare_core_errors_are_conversion_tests_only() {
    for (core, code) in [
        (
            cribra::StreamError::StorageRequirementOverflow,
            StreamErrorCode::StorageRequirementOverflow,
        ),
        (
            cribra::StreamError::SourceOffsetOverflow,
            StreamErrorCode::SourceOffsetOverflow,
        ),
        (
            cribra::StreamError::CoordinateOverflow,
            StreamErrorCode::CoordinateOverflow,
        ),
        (
            cribra::StreamError::ExecutionInvariantViolated,
            StreamErrorCode::ExecutionInvariantViolated,
        ),
    ] {
        assert!(
            matches!(StreamError::from(core), StreamError::Rejected { code: actual, limit: None, required: None, .. } if actual == code)
        );
    }
}
