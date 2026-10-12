//! External-consumer contract: public types suffice, with no internal records.
use cribra::{Rule, Scanner, Severity, StreamError, StreamLimits, StreamSession};

#[test]
fn public_session_is_send_and_returns_the_existing_report() {
    fn assert_send<T: Send>() {}
    assert_send::<StreamSession>();
    let scanner = Scanner::builder()
        .rule(Rule::literal("credential", "SECRET", Severity::High))
        .build()
        .unwrap();
    let source = "é\r\nSECRET ABCD-EFGH-IJKL-MNOP";
    let expected = scanner.scan([((), source)]);
    let limits = StreamLimits::default()
        .with_max_candidates(Some(1))
        .with_max_retained_results(Some(2));
    let mut session = scanner.stream_with_limits(limits).unwrap();
    for byte in source.as_bytes() {
        session.push(std::slice::from_ref(byte)).unwrap();
    }
    let report = session.finish().unwrap();
    assert_eq!(&report, expected.single_report().unwrap());
    assert_eq!(report.len(), 1);
    assert_eq!(report.candidate_len(), 1);
}

#[test]
fn public_failure_contract_never_exposes_partial_completion() {
    let scanner = Scanner::builder()
        .rule(Rule::literal("credential", "SECRET", Severity::High))
        .build()
        .unwrap();
    let mut session = scanner
        .stream_with_limits(StreamLimits::default().with_max_retained_results(Some(1)))
        .unwrap();
    session.push(b"SECRET ").unwrap();
    assert_eq!(
        session.push(b"SECRET"),
        Err(StreamError::ResultLimitExceeded { limit: 1 })
    );
    assert_eq!(session.push(b""), Err(StreamError::NotActive));
    assert_eq!(session.finish(), Err(StreamError::NotActive));
}
