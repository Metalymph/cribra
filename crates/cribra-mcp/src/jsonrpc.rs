//! JSON-RPC 2.0 envelope parsing and request identifiers.
//!
//! Parsing is deliberately structural: it classifies an input line as a
//! request, a notification, or a fixed protocol error. Method-specific
//! validation belongs to the protocol layer.

use serde::Serialize;
use serde_json::Value;

use crate::error::{BATCH_NOT_SUPPORTED, INVALID_REQUEST, PARSE_ERROR, RpcError};
use crate::limits::MAX_REQUEST_ID_BYTES;

/// A JSON-RPC request identifier.
///
/// The MCP schemas allow strings and integers. Integers outside `i64` are
/// rejected instead of being lossy-converted.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
#[serde(untagged)]
pub enum RequestId {
    /// An integer identifier.
    Number(i64),
    /// A string identifier.
    String(String),
}

impl RequestId {
    /// Parses a request identifier from a JSON value.
    ///
    /// String identifiers are bounded to [`MAX_REQUEST_ID_BYTES`] UTF-8 bytes.
    #[must_use]
    pub fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::String(text) if text.len() <= MAX_REQUEST_ID_BYTES => {
                Some(Self::String(text.clone()))
            }
            Value::Number(number) => {
                if let Some(value) = number.as_i64() {
                    Some(Self::Number(value))
                } else {
                    number
                        .as_u64()
                        .filter(|value| *value <= i64::MAX as u64)
                        .map(|value| Self::Number(value as i64))
                }
            }
            _ => None,
        }
    }

    /// Returns the JSON representation of this identifier.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::Number(value) => Value::from(*value),
            Self::String(value) => Value::from(value.clone()),
        }
    }
}

/// A parsed incoming message.
#[derive(Clone, Debug)]
pub enum ParsedMessage {
    /// A request that expects one response.
    Request {
        /// The request identifier.
        id: RequestId,
        /// The method name.
        method: String,
        /// The raw `params` member, if any.
        params: Option<Value>,
    },
    /// A notification that expects no response.
    Notification {
        /// The method name.
        method: String,
        /// The raw `params` member, if any.
        params: Option<Value>,
    },
    /// A structurally invalid message, with a fixed protocol error.
    Invalid {
        /// The safest available identifier, if the envelope exposed one.
        id: Option<RequestId>,
        /// The fixed error to emit.
        error: RpcError,
    },
}

/// Parses one newline-delimited JSON-RPC message.
#[must_use]
pub fn parse_message(line: &[u8]) -> ParsedMessage {
    let value = match serde_json::from_slice::<Value>(line) {
        Ok(value) => value,
        Err(_) => {
            return ParsedMessage::Invalid {
                id: None,
                error: PARSE_ERROR,
            };
        }
    };
    parse_value(value)
}

fn parse_value(value: Value) -> ParsedMessage {
    if value.is_array() {
        return ParsedMessage::Invalid {
            id: None,
            error: BATCH_NOT_SUPPORTED,
        };
    }

    let Value::Object(mut object) = value else {
        return ParsedMessage::Invalid {
            id: None,
            error: INVALID_REQUEST,
        };
    };

    let raw_id = object.remove("id");
    let id = match raw_id.as_ref() {
        None => None,
        Some(value) => match RequestId::from_value(value) {
            Some(id) => Some(id),
            None => {
                return ParsedMessage::Invalid {
                    id: None,
                    error: INVALID_REQUEST,
                };
            }
        },
    };

    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return ParsedMessage::Invalid {
            id,
            error: INVALID_REQUEST,
        };
    }

    let Some(Value::String(method)) = object.remove("method") else {
        return ParsedMessage::Invalid {
            id,
            error: INVALID_REQUEST,
        };
    };

    let params = object.remove("params");
    match id {
        Some(id) => ParsedMessage::Request { id, method, params },
        None => ParsedMessage::Notification { method, params },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_request_and_notification() {
        let request = parse_message(
            br#"{"jsonrpc":"2.0","id":"abc","method":"tools/list","params":{"_meta":{}}}"#,
        );
        match request {
            ParsedMessage::Request { id, method, params } => {
                assert_eq!(id, RequestId::String("abc".to_owned()));
                assert_eq!(method, "tools/list");
                assert!(params.is_some());
            }
            other => panic!("unexpected parse result: {other:?}"),
        }

        let notification =
            parse_message(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        assert!(matches!(notification, ParsedMessage::Notification { .. }));
    }

    #[test]
    fn rejects_integer_overflow_and_null_id() {
        for line in [
            br#"{"jsonrpc":"2.0","id":18446744073709551615,"method":"ping"}"#.as_slice(),
            br#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#.as_slice(),
            br#"{"jsonrpc":"2.0","id":1.5,"method":"ping"}"#.as_slice(),
        ] {
            assert!(matches!(parse_message(line), ParsedMessage::Invalid { .. }));
        }
    }

    #[test]
    fn rejects_oversized_string_id() {
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":"{}","method":"ping"}}"#,
            "a".repeat(MAX_REQUEST_ID_BYTES + 1)
        );
        assert!(matches!(
            parse_message(line.as_bytes()),
            ParsedMessage::Invalid { .. }
        ));
    }

    #[test]
    fn classifies_batch_and_parse_errors() {
        assert!(matches!(
            parse_message(br#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#),
            ParsedMessage::Invalid {
                error: RpcError { code: -32600, .. },
                ..
            }
        ));
        assert!(matches!(
            parse_message(b"{"),
            ParsedMessage::Invalid {
                error: RpcError { code: -32700, .. },
                ..
            }
        ));
    }
}
