//! Static tool contracts and M2 execution entry points.
//!
//! Exactly two tools are advertised. `cribra_scan` returns privacy-safe Core
//! finding and candidate metadata without source fragments. `cribra_transform`
//! returns Core-produced transformed source; unchanged source regions remain
//! visible and the result is not claimed to be anonymized.
//!
//! Argument validation and Core execution live in [`crate::tool_arguments`]
//! and [`crate::core_adapter`]. This module owns the MCP result envelopes and
//! the exact response-size check.

use std::io::{self, Write};

use serde_json::{Value, json};

use crate::core_adapter;
use crate::jsonrpc::RequestId;
use crate::limits::MAX_RESPONSE_BYTES;
use crate::tool_arguments::{ErrorCode, ToolError, parse_scan, parse_transform};

/// Embedded `cribra_scan` input schema.
pub const SCAN_INPUT_SCHEMA: &str = include_str!("../schemas/cribra_scan.input.json");
/// Embedded `cribra_scan` structured output schema.
pub const SCAN_OUTPUT_SCHEMA: &str = include_str!("../schemas/cribra_scan.output.json");
/// Embedded `cribra_transform` input schema.
pub const TRANSFORM_INPUT_SCHEMA: &str = include_str!("../schemas/cribra_transform.input.json");
/// Embedded `cribra_transform` structured output schema.
pub const TRANSFORM_OUTPUT_SCHEMA: &str = include_str!("../schemas/cribra_transform.output.json");

/// The only tool names advertised by this build.
pub const TOOL_NAMES: [&str; 2] = ["cribra_scan", "cribra_transform"];

const SCAN_DESCRIPTION: &str = "Scan UTF-8 text with Cribra Core built-in rules and optional bounded custom regex rules. Returns privacy-safe finding and candidate metadata only; the source text and matched substrings are never returned.";
const TRANSFORM_DESCRIPTION: &str = "Scan UTF-8 text with Cribra Core and apply the redact or template transformation. Returns Core-produced transformed source in structuredContent.output and as the first text content block. Unchanged source regions remain visible; this is not anonymization.";

/// A successful Core tool projection.
#[derive(Debug)]
pub(crate) struct ToolSuccess {
    /// Structured content conforming to the tool's output schema.
    pub structured: Value,
    /// Source-safe text content for the first MCP content block.
    pub text: String,
}

/// Returns whether `name` belongs to the static contract.
#[must_use]
pub fn is_known_tool(name: &str) -> bool {
    TOOL_NAMES.contains(&name)
}

/// Parses an embedded schema.
pub fn parse_schema(source: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(source)
}

/// Builds the exact tool list advertised for `tools/list`.
pub fn tools_array() -> Result<Vec<Value>, serde_json::Error> {
    let scan_input = parse_schema(SCAN_INPUT_SCHEMA)?;
    let scan_output = parse_schema(SCAN_OUTPUT_SCHEMA)?;
    let transform_input = parse_schema(TRANSFORM_INPUT_SCHEMA)?;
    let transform_output = parse_schema(TRANSFORM_OUTPUT_SCHEMA)?;

    Ok(vec![
        json!({
            "name": "cribra_scan",
            "title": "Cribra scan",
            "description": SCAN_DESCRIPTION,
            "inputSchema": scan_input,
            "outputSchema": scan_output,
            "annotations": {
                "title": "Cribra scan",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false,
            },
        }),
        json!({
            "name": "cribra_transform",
            "title": "Cribra transform",
            "description": TRANSFORM_DESCRIPTION,
            "inputSchema": transform_input,
            "outputSchema": transform_output,
            "annotations": {
                "title": "Cribra transform",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false,
            },
        }),
    ])
}

/// Executes one known tool call and returns a complete MCP tool result.
///
/// The returned value contains `content`, `isError` and `structuredContent`.
/// It never contains source text for `cribra_scan`; `cribra_transform`
/// intentionally contains the Core-produced transformed source.
#[must_use]
pub fn execute(name: &str, arguments: &Value) -> Value {
    let outcome = match name {
        "cribra_scan" => parse_scan(arguments).and_then(core_adapter::scan),
        "cribra_transform" => parse_transform(arguments).and_then(core_adapter::transform),
        _ => Err(ToolError::new(ErrorCode::InternalError)),
    };

    match outcome {
        Ok(success) => success_result(success),
        Err(error) => error_result(error.code()),
    }
}

/// Returns a fixed tool-shaped internal error result.
#[must_use]
pub fn internal_error_result() -> Value {
    error_result(ErrorCode::InternalError)
}

/// Returns a fixed tool-shaped response-size error result.
#[must_use]
pub fn response_too_large_result() -> Value {
    error_result(ErrorCode::ResponseTooLarge)
}

/// Returns whether the complete encoded JSON-RPC response fits the transport
/// limit.
///
/// This is exact, not an estimate. It serializes the already era-shaped tool
/// result once with a counting writer, serializes the request identifier once,
/// and adds the fixed JSON-RPC success envelope:
///
/// `{"jsonrpc":"2.0","id":` + id + `,"result":` + result + `}` + newline.
#[must_use]
pub fn response_fits(id: &RequestId, result: &Value) -> bool {
    const PREFIX: &str = "{\"jsonrpc\":\"2.0\",\"id\":";
    const SEPARATOR: &str = ",\"result\":";

    let Ok(encoded_id) = serde_json::to_string(id) else {
        return false;
    };
    let mut counter = CountingWriter::new(MAX_RESPONSE_BYTES);
    if serde_json::to_writer(&mut counter, result).is_err() {
        return false;
    }

    PREFIX
        .len()
        .saturating_add(encoded_id.len())
        .saturating_add(SEPARATOR.len())
        .saturating_add(counter.len)
        .saturating_add(2)
        <= MAX_RESPONSE_BYTES
}

fn success_result(success: ToolSuccess) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": success.text,
            }
        ],
        "isError": false,
        "structuredContent": success.structured,
    })
}

fn error_result(code: ErrorCode) -> Value {
    let message = code.message();
    json!({
        "content": [
            {
                "type": "text",
                "text": message,
            }
        ],
        "isError": true,
        "structuredContent": {
            "status": "error",
            "error": {
                "code": code.as_str(),
                "message": message,
            }
        }
    })
}

/// A writer that counts bytes and stops serialization once a limit is crossed.
struct CountingWriter {
    len: usize,
    limit: usize,
}

impl CountingWriter {
    const fn new(limit: usize) -> Self {
        Self { len: 0, limit }
    }
}

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.len = self.len.saturating_add(buffer.len());
        if self.len > self.limit {
            return Err(io::Error::other("response size limit exceeded"));
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Era, Reply, shape_success};

    fn encoded_reply_len(id: &RequestId, result: &Value) -> usize {
        serde_json::to_vec(&Reply::success(id.clone(), result.clone(), Era::Modern).to_value())
            .expect("reply serializes")
            .len()
            + 1
    }

    #[test]
    fn execute_returns_schema_shaped_success_envelopes() {
        let scan = execute("cribra_scan", &json!({"source": "hello"}));
        assert_eq!(scan["isError"], false);
        assert_eq!(scan["content"][0]["type"], "text");
        assert_eq!(
            scan["content"][0]["text"],
            "Scan complete: 0 findings, 0 candidates."
        );
        assert_eq!(scan["structuredContent"]["status"], "ok");

        let transform = execute(
            "cribra_transform",
            &json!({"source": "hello", "mode": "redact"}),
        );
        assert_eq!(transform["isError"], false);
        assert_eq!(transform["structuredContent"]["status"], "ok");
        assert_eq!(
            transform["content"][0]["text"],
            transform["structuredContent"]["output"]
        );
    }

    #[test]
    fn execute_returns_fixed_source_free_error_envelopes() {
        const CANARY: &str = "CRIBRA_ERROR_SOURCE_CANARY";
        let result = execute("cribra_scan", &json!({"source": CANARY, "extra": true}));
        assert_eq!(result["isError"], true);
        assert_eq!(result["structuredContent"]["status"], "error");
        assert_eq!(
            result["structuredContent"]["error"]["code"],
            "invalid_arguments"
        );
        assert_eq!(
            result["content"][0]["text"],
            result["structuredContent"]["error"]["message"]
        );
        assert!(!result.to_string().contains(CANARY));

        let unknown = execute("not_a_tool", &json!({"source": CANARY}));
        assert_eq!(unknown["isError"], true);
        assert_eq!(
            unknown["structuredContent"]["error"]["code"],
            "internal_error"
        );
        assert!(!unknown.to_string().contains(CANARY));
    }

    #[test]
    fn response_fits_is_exact_for_the_complete_envelope() {
        // Compute the fixed envelope overhead with an oversized payload, then
        // shrink the payload so the complete encoded reply is exactly the limit.
        let id = RequestId::Number(42);
        let mut oversized = json!({"payload": "x".repeat(MAX_RESPONSE_BYTES)});
        shape_success(Era::Modern, &mut oversized);
        let overhead = encoded_reply_len(&id, &oversized) - MAX_RESPONSE_BYTES;
        assert!(overhead < MAX_RESPONSE_BYTES);

        let mut exact = json!({"payload": "x".repeat(MAX_RESPONSE_BYTES - overhead)});
        shape_success(Era::Modern, &mut exact);
        assert_eq!(encoded_reply_len(&id, &exact), MAX_RESPONSE_BYTES);
        assert!(response_fits(&id, &exact));

        let mut over = json!({"payload": "x".repeat(MAX_RESPONSE_BYTES - overhead + 1)});
        shape_success(Era::Modern, &mut over);
        assert_eq!(encoded_reply_len(&id, &over), MAX_RESPONSE_BYTES + 1);
        assert!(!response_fits(&id, &over));
    }

    #[test]
    fn response_fits_accounts_for_escaped_request_ids() {
        let id = RequestId::String("\"\\\n😀".to_owned());
        let mut oversized = json!({"payload": "x".repeat(MAX_RESPONSE_BYTES)});
        shape_success(Era::Modern, &mut oversized);
        let overhead = encoded_reply_len(&id, &oversized) - MAX_RESPONSE_BYTES;

        let mut exact = json!({"payload": "x".repeat(MAX_RESPONSE_BYTES - overhead)});
        shape_success(Era::Modern, &mut exact);
        assert_eq!(encoded_reply_len(&id, &exact), MAX_RESPONSE_BYTES);
        assert!(response_fits(&id, &exact));

        let mut over = json!({"payload": "x".repeat(MAX_RESPONSE_BYTES - overhead + 1)});
        shape_success(Era::Modern, &mut over);
        assert!(!response_fits(&id, &over));
    }

    #[test]
    fn response_fits_accounts_for_modern_era_metadata() {
        let id = RequestId::Number(1);
        let mut result = json!({"payload": "x"});
        let legacy_len =
            serde_json::to_vec(&Reply::success(id.clone(), result.clone(), Era::Legacy).to_value())
                .expect("legacy serializes")
                .len()
                + 1;
        shape_success(Era::Modern, &mut result);
        let modern_len = encoded_reply_len(&id, &result);
        assert!(
            modern_len > legacy_len,
            "modern metadata must be included in the size check"
        );
        assert!(response_fits(&id, &result));
    }

    #[test]
    fn fallback_results_are_bounded_and_source_free() {
        assert_eq!(
            internal_error_result()["structuredContent"]["error"]["code"],
            "internal_error"
        );
        assert_eq!(
            response_too_large_result()["structuredContent"]["error"]["code"],
            "response_too_large"
        );
        assert!(
            serde_json::to_vec(&response_too_large_result())
                .expect("serializes")
                .len()
                < 1024
        );
    }
}
