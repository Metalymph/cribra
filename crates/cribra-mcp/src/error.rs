//! Fixed, privacy-safe protocol errors.
//!
//! Messages in this module are constants. Request identifiers, tool names,
//! method names, argument values, source text, patterns and key material are
//! never interpolated into a diagnostic.

use serde_json::{Value, json};

use crate::jsonrpc::RequestId;

/// A JSON-RPC error with an optional, explicitly constructed `data` member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: &'static str,
    pub data: Option<Value>,
}

impl RpcError {
    pub const fn new(code: i64, message: &'static str) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

pub const PARSE_ERROR: RpcError = RpcError::new(-32700, "Parse error");
pub const INVALID_REQUEST: RpcError = RpcError::new(-32600, "Invalid request");
pub const METHOD_NOT_FOUND: RpcError = RpcError::new(-32601, "Method not found");
pub const INVALID_PARAMS: RpcError = RpcError::new(-32602, "Invalid params");
pub const INTERNAL_ERROR: RpcError = RpcError::new(-32603, "Internal error");
pub const OVERLOADED: RpcError = RpcError::new(-32603, "Server overloaded");
pub const MISSING_META: RpcError = RpcError::new(-32602, "Missing required _meta fields");
pub const INVALID_INITIALIZE: RpcError = RpcError::new(-32602, "Invalid initialize parameters");
pub const INITIALIZE_ALREADY_COMPLETED: RpcError =
    RpcError::new(-32600, "Initialize already completed");
pub const NOT_INITIALIZED: RpcError = RpcError::new(-32600, "Server not initialized");
pub const UNKNOWN_TOOL: RpcError = RpcError::new(-32602, "Unknown tool");
pub const INVALID_TOOL_CALL: RpcError = RpcError::new(-32602, "Invalid tool call parameters");
pub const MESSAGE_TOO_LARGE: RpcError =
    RpcError::new(-32600, "Request exceeds maximum message size");
pub const BATCH_NOT_SUPPORTED: RpcError = RpcError::new(-32600, "Batch requests are not supported");
pub const UNSUPPORTED_PROTOCOL_VERSION: RpcError =
    RpcError::new(-32022, "Unsupported protocol version");

/// Builds the required `-32022` payload. `requested` is bounded and sanitized
/// before it is included.
pub fn unsupported_protocol_version(requested: &str) -> RpcError {
    UNSUPPORTED_PROTOCOL_VERSION.with_data(json!({
        "supported": [crate::protocol::MODERN_VERSION],
        "requested": sanitize_version(requested),
    }))
}

/// Keeps only characters that are safe to place in a diagnostic field and
/// bounds the result. The value is still client-provided, so it is never
/// treated as trusted input.
pub fn sanitize_version(version: &str) -> String {
    let mut out = String::with_capacity(version.len().min(64));
    for ch in version.chars().take(64) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "invalid".to_owned()
    } else {
        out
    }
}

/// Serialized envelope for a JSON-RPC error response.
///
/// MCP `2026-07-28` omits `id` when the request identifier could not be read;
/// this is also what keeps malformed-message errors valid against the pinned
/// official schema, whose `id` member is optional but not nullable.
pub fn error_value(id: Option<&RequestId>, error: &RpcError) -> Value {
    let mut inner = json!({
        "code": error.code,
        "message": error.message,
    });
    if let Some(data) = &error.data {
        inner["data"] = data.clone();
    }
    let mut value = json!({
        "jsonrpc": "2.0",
        "error": inner,
    });
    if let Some(id) = id {
        value["id"] = id.to_value();
    }
    value
}
