//! Static tool contracts for M1.
//!
//! Exactly two tool contracts are advertised. Their handlers are deliberate
//! stubs: M1 qualifies the protocol surface, not scan or transform behavior.
//! Every call returns a fixed `isError: true` result so no functional
//! interoperability is implied.

use serde_json::{Value, json};

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

const SCAN_DESCRIPTION: &str = "M1 protocol contract only. Returns a fixed isError result; scanning is not implemented in this build. M2 will return privacy-safe finding metadata without source fragments.";
const TRANSFORM_DESCRIPTION: &str = "M1 protocol contract only. Returns a fixed isError result; transformation is not implemented in this build. M2 will return Core-produced transformed source in structuredContent.output and as a compatible text block; unchanged source parts remain visible.";

/// Returns whether `name` belongs to the static M1 contract.
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

/// The fixed tool result used by the M1 stub handler.
#[must_use]
pub fn stub_tool_result() -> Value {
    const MESSAGE: &str = "Tool execution is unavailable in this M1 build";
    json!({
        "content": [
            {
                "type": "text",
                "text": MESSAGE,
            }
        ],
        "isError": true,
        "structuredContent": {
            "status": "error",
            "error": {
                "code": "internal_error",
                "message": MESSAGE,
            }
        }
    })
}
