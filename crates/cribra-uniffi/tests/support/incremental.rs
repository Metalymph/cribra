use serde_json::{Value, json};

pub fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/incremental.json")).unwrap()
}

pub fn core_error(error: cribra::IncrementalError, terminal: bool) -> Value {
    use cribra::IncrementalError as E;
    let code = match error {
        E::NotActive => "NotActive",
        E::StaleRevision { .. } => "StaleRevision",
        E::InvalidRange => "InvalidRange",
        E::InvalidUtf8Boundary => "InvalidUtf8Boundary",
        E::RevisionOverflow => "RevisionOverflow",
        E::DocumentLengthOverflow => "DocumentLengthOverflow",
        E::CandidateLimitExceeded { .. } => "CandidateLimitExceeded",
        E::CacheLimitExceeded { .. } => "CacheLimitExceeded",
        E::ResultLimitExceeded { .. } => "ResultLimitExceeded",
        E::TransactionStorageLimitExceeded { .. } => "TransactionStorageLimitExceeded",
        E::ResourceAccountingOverflow => "ResourceAccountingOverflow",
        E::ExecutionInvariantViolated => "ExecutionInvariantViolated",
        _ => panic!("unknown Core error"),
    };
    let (expected, actual) = match error {
        E::StaleRevision { expected, actual } => (Some(expected), Some(actual)),
        _ => (None, None),
    };
    let (limit, required) = match error {
        E::CandidateLimitExceeded { limit }
        | E::CacheLimitExceeded { limit }
        | E::ResultLimitExceeded { limit } => (Some(limit), None),
        E::TransactionStorageLimitExceeded { required, limit } => (Some(limit), Some(required)),
        _ => (None, None),
    };
    json!({"code":code,"terminal":terminal,"expectedRevision":expected,"actualRevision":actual,
        "limit":limit,"required":required,"detail":error.to_string()})
}
