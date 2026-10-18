//! Independent Core error oracle for the real Swift streaming consumer.

use cribra::{Rule, Scanner, Severity, StreamError, StreamLimits};
use serde_json::{Value, json};

fn snapshot(error: StreamError) -> Value {
    let (code, limit, required) = match error {
        StreamError::UnsupportedRuleSet => ("UnsupportedRuleSet", None, None),
        StreamError::NotActive => ("NotActive", None, None),
        StreamError::InvalidUtf8 => ("InvalidUtf8", None, None),
        StreamError::IncompleteUtf8 => ("IncompleteUtf8", None, None),
        StreamError::CandidateLimitExceeded { limit } => {
            ("CandidateLimitExceeded", Some(limit), None)
        }
        StreamError::ResultLimitExceeded { limit } => ("ResultLimitExceeded", Some(limit), None),
        StreamError::StorageLimitExceeded { required, limit } => {
            ("StorageLimitExceeded", Some(limit), Some(required))
        }
        _ => panic!("unexpected error in executable streaming fixture: {error}"),
    };
    json!({ "code": code, "limit": limit, "required": required, "detail": error.to_string() })
}

pub fn errors() -> Value {
    let scanner = Scanner::builder()
        .rule(Rule::literal("literal", "SECRET", Severity::High))
        .build()
        .unwrap();
    let mut invalid = scanner.stream().unwrap();
    invalid.push(b"SECRET ").unwrap();
    let invalid_error = invalid.push(&[0xff]).unwrap_err();
    let mut incomplete = scanner.stream().unwrap();
    incomplete.push(&[0xf0, 0x9f]).unwrap();
    let mut candidate = scanner
        .stream_with_limits(StreamLimits::default().with_max_candidates(Some(1)))
        .unwrap();
    candidate.push(b"SECRET ").unwrap();
    let mut result = scanner
        .stream_with_limits(StreamLimits::default().with_max_retained_results(Some(1)))
        .unwrap();
    result.push(b"SECRET ").unwrap();
    let empty = Scanner::builder().build().unwrap();
    let mut eof_result = empty
        .stream_with_limits(StreamLimits::default().with_max_retained_results(Some(0)))
        .unwrap();
    eof_result.push(b"ABCD-EFGH-IJKL-MNOP").unwrap();
    let bounded_eof = Scanner::builder()
        .rule(Rule::pattern("eof", r"abc\z", Severity::High).unwrap())
        .build()
        .unwrap();
    let mut eof_candidate = bounded_eof
        .stream_with_limits(StreamLimits::default().with_max_candidates(Some(0)))
        .unwrap();
    eof_candidate.push(b"abc").unwrap();
    json!({
        "unsupported": snapshot(Scanner::default().stream().unwrap_err()),
        "storage": snapshot(scanner.stream_with_limits(StreamLimits::default().with_max_stream_storage_bytes(Some(0))).unwrap_err()),
        "notActive": snapshot(StreamError::NotActive),
        "invalidUtf8": snapshot(invalid_error),
        "incompleteUtf8": snapshot(incomplete.finish().unwrap_err()),
        "candidate": snapshot(candidate.push(b"SECRET ").unwrap_err()),
        "result": snapshot(result.push(b"SECRET ").unwrap_err()),
        "eofResult": snapshot(eof_result.finish().unwrap_err()),
        "eofCandidate": snapshot(eof_candidate.finish().unwrap_err()),
    })
}
