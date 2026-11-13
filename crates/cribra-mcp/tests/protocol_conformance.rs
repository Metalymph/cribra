//! Protocol shaping and pinning checks against the official MCP schemas.

use std::sync::LazyLock;

use cribra_mcp::jsonrpc::parse_message;
use cribra_mcp::protocol::{Dispatch, MODERN_VERSION, Reply, ReplyOutcome, Session, shape_success};
use cribra_mcp::tooling;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

static MODERN_SCHEMA: &str = include_str!("schemas/2026-07-28.json");
static LEGACY_SCHEMA: &str = include_str!("schemas/2025-11-25.json");

static MODERN_VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    build_message_validator(MODERN_SCHEMA).expect("modern official schema compiles")
});
static LEGACY_VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    build_message_validator(LEGACY_SCHEMA).expect("legacy official schema compiles")
});

fn build_message_validator(source: &str) -> Result<jsonschema::Validator, String> {
    let mut schema: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
    let object = schema
        .as_object_mut()
        .ok_or_else(|| "schema root is not an object".to_owned())?;
    object.insert("$ref".to_owned(), json!("#/$defs/JSONRPCMessage"));
    jsonschema::validator_for(&schema).map_err(|error| error.to_string())
}

fn validate(version: &str, value: &Value) {
    let validator = if version == MODERN_VERSION {
        &*MODERN_VALIDATOR
    } else {
        &*LEGACY_VALIDATOR
    };
    if validator.is_valid(value) {
        return;
    }
    let mut details = String::new();
    for error in validator.iter_errors(value) {
        details.push_str(&format!("\n  {error} at {}", error.instance_path()));
    }
    panic!("message violates {version} schema: {value}{details}");
}

fn dispatch(session: &mut Session, request: Value) -> Option<Value> {
    let line = serde_json::to_vec(&request).expect("request serializes");
    let parsed = parse_message(&line);
    match session.dispatch(parsed) {
        Dispatch::Reply(reply) => Some(reply.to_value()),
        Dispatch::ToolCall(call) => {
            let mut result = tooling::stub_tool_result();
            shape_success(call.era, &mut result);
            Some(
                Reply {
                    id: Some(call.id),
                    outcome: ReplyOutcome::Success(result),
                }
                .to_value(),
            )
        }
        Dispatch::None => None,
    }
}

fn meta(version: &str) -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": version,
        "io.modelcontextprotocol/clientCapabilities": {},
    })
}

fn modern_request(id: u64, method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

fn modern_meta_request(id: u64, method: &str) -> Value {
    modern_request(id, method, json!({"_meta": meta(MODERN_VERSION)}))
}

fn legacy_initialize(id: u64, version: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": version,
            "capabilities": {},
            "clientInfo": {"name": "protocol-test", "version": "1.0.0"},
        },
    })
}

fn tool_names(result: &Value) -> Vec<String> {
    result["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name").to_owned())
        .collect()
}

#[test]
fn official_schemas_are_pinned_by_hash() {
    assert_eq!(
        format!("{:x}", Sha256::digest(MODERN_SCHEMA.as_bytes())),
        "ef70b61f99b6d2e5e3b46863822eab08dff6a45bedc7a08914e0e5b133f40203"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(LEGACY_SCHEMA.as_bytes())),
        "1ffe4c5577974012f5fa02af14ea88df4b7146679df1abaaad497c8d9230ca8a"
    );
}

#[test]
fn advertised_versions_are_exactly_the_wire_versions_under_test() {
    let mut session = Session::new();
    let response = dispatch(
        &mut session,
        modern_request(1, "server/discover", json!({"_meta": meta(MODERN_VERSION)})),
    )
    .expect("discover response");
    validate(MODERN_VERSION, &response);

    assert_eq!(
        response["result"]["supportedVersions"],
        json!([MODERN_VERSION])
    );
    assert_eq!(response["result"]["resultType"], "complete");
    assert_eq!(response["result"]["ttlMs"], 0);
    assert_eq!(response["result"]["cacheScope"], "private");
    assert_eq!(response["result"]["capabilities"], json!({"tools": {}}));
}

#[test]
fn modern_meta_validation_and_version_errors_are_exact() {
    let mut session = Session::new();

    let missing = dispatch(
        &mut session,
        modern_request(1, "server/discover", json!({})),
    )
    .expect("missing meta response");
    validate(MODERN_VERSION, &missing);
    assert_eq!(missing["error"]["code"], -32602);
    assert_eq!(missing["error"]["message"], "Missing required _meta fields");

    let missing_capabilities = dispatch(
        &mut session,
        modern_request(
            2,
            "server/discover",
            json!({"_meta": {"io.modelcontextprotocol/protocolVersion": MODERN_VERSION}}),
        ),
    )
    .expect("missing capabilities response");
    validate(MODERN_VERSION, &missing_capabilities);
    assert_eq!(missing_capabilities["error"]["code"], -32602);

    let malformed_client_info = dispatch(
        &mut session,
        modern_request(
            3,
            "server/discover",
            json!({"_meta": {
                "io.modelcontextprotocol/protocolVersion": MODERN_VERSION,
                "io.modelcontextprotocol/clientCapabilities": {},
                "io.modelcontextprotocol/clientInfo": {"name": 1},
            }}),
        ),
    )
    .expect("malformed client info response");
    validate(MODERN_VERSION, &malformed_client_info);
    assert_eq!(malformed_client_info["error"]["code"], -32602);

    let unsupported = dispatch(
        &mut session,
        modern_request(4, "server/discover", json!({"_meta": meta("1900-01-01")})),
    )
    .expect("unsupported version response");
    validate(MODERN_VERSION, &unsupported);
    assert_eq!(unsupported["error"]["code"], -32022);
    assert_eq!(
        unsupported["error"]["message"],
        "Unsupported protocol version"
    );
    assert_eq!(
        unsupported["error"]["data"]["supported"],
        json!([MODERN_VERSION])
    );
    assert_eq!(unsupported["error"]["data"]["requested"], "1900-01-01");
}

#[test]
fn modern_initialize_and_ping_are_absent() {
    let mut session = Session::new();
    for (id, method) in [(1, "initialize"), (2, "ping")] {
        let response = dispatch(&mut session, modern_meta_request(id, method)).expect("response");
        validate(MODERN_VERSION, &response);
        assert_eq!(response["error"]["code"], -32601);
        assert_eq!(response["error"]["message"], "Method not found");
    }
}

#[test]
fn modern_tools_surface_is_two_static_contracts() {
    let mut session = Session::new();
    let list = dispatch(&mut session, modern_meta_request(1, "tools/list")).expect("tool list");
    validate(MODERN_VERSION, &list);
    assert_eq!(list["result"]["resultType"], "complete");
    assert_eq!(list["result"]["ttlMs"], 0);
    assert_eq!(list["result"]["cacheScope"], "private");
    assert_eq!(tool_names(&list["result"]), tooling::TOOL_NAMES);

    for (id, name) in [(2, "cribra_scan"), (3, "cribra_transform")] {
        let call = dispatch(
            &mut session,
            modern_request(
                id,
                "tools/call",
                json!({
                    "_meta": meta(MODERN_VERSION),
                    "name": name,
                    "arguments": {"source": "not implemented"},
                }),
            ),
        )
        .expect("tool call");
        validate(MODERN_VERSION, &call);
        assert_eq!(call["result"]["resultType"], "complete");
        assert_eq!(call["result"]["isError"], true);
        assert_eq!(call["result"]["structuredContent"]["status"], "error");
        assert_eq!(
            call["result"]["structuredContent"]["error"]["code"],
            "internal_error"
        );
    }

    let unknown = dispatch(
        &mut session,
        modern_request(
            4,
            "tools/call",
            json!({"_meta": meta(MODERN_VERSION), "name": "unknown"}),
        ),
    )
    .expect("unknown tool response");
    validate(MODERN_VERSION, &unknown);
    assert_eq!(unknown["error"]["code"], -32602);
    assert_eq!(unknown["error"]["message"], "Unknown tool");

    for (id, params) in [
        (
            5,
            json!({"_meta": meta(MODERN_VERSION), "name": "cribra_scan", "arguments": []}),
        ),
        (
            6,
            json!({"_meta": meta(MODERN_VERSION), "name": "cribra_scan", "inputResponses": {}}),
        ),
        (
            7,
            json!({"_meta": meta(MODERN_VERSION), "name": 7, "arguments": {}}),
        ),
    ] {
        let invalid = dispatch(&mut session, modern_request(id, "tools/call", params))
            .expect("invalid call response");
        validate(MODERN_VERSION, &invalid);
        assert_eq!(invalid["error"]["code"], -32602);
        assert_eq!(invalid["error"]["message"], "Invalid tool call parameters");
    }
}

#[test]
fn legacy_lifecycle_and_shaping_are_exact() {
    let mut session = Session::new();

    let before = dispatch(
        &mut session,
        json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}),
    )
    .expect("pre-init response");
    validate("2025-11-25", &before);
    assert_eq!(before["error"]["code"], -32602);

    let initialized =
        dispatch(&mut session, legacy_initialize(2, "2024-11-05")).expect("initialize response");
    validate("2025-11-25", &initialized);
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    assert!(initialized["result"].get("resultType").is_none());
    assert!(initialized["result"].get("ttlMs").is_none());

    let before_notification = dispatch(
        &mut session,
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}),
    )
    .expect("not-initialized response");
    validate("2025-11-25", &before_notification);
    assert_eq!(before_notification["error"]["code"], -32600);
    assert_eq!(
        before_notification["error"]["message"],
        "Server not initialized"
    );

    assert!(
        dispatch(
            &mut session,
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .is_none()
    );

    let ping = dispatch(
        &mut session,
        json!({"jsonrpc": "2.0", "id": 4, "method": "ping"}),
    )
    .expect("legacy ping");
    validate("2025-11-25", &ping);
    assert_eq!(ping["result"], json!({}));

    let list = dispatch(
        &mut session,
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list"}),
    )
    .expect("legacy tools list");
    validate("2025-11-25", &list);
    assert_eq!(tool_names(&list["result"]), tooling::TOOL_NAMES);
    assert!(list["result"].get("resultType").is_none());
    assert!(list["result"].get("ttlMs").is_none());

    let call = dispatch(
        &mut session,
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {"name": "cribra_scan", "arguments": {"source": "x"}},
        }),
    )
    .expect("legacy tool call");
    validate("2025-11-25", &call);
    assert_eq!(call["result"]["isError"], true);
    assert!(call["result"].get("resultType").is_none());

    let second = dispatch(&mut session, legacy_initialize(7, "2025-11-25"))
        .expect("second initialize response");
    validate("2025-11-25", &second);
    assert_eq!(second["error"]["code"], -32600);
    assert_eq!(second["error"]["message"], "Initialize already completed");
}

#[test]
fn legacy_initialize_validation_and_initialized_ordering() {
    let mut session = Session::new();
    assert!(
        dispatch(
            &mut session,
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .is_none()
    );
    assert_eq!(
        session.legacy_state(),
        cribra_mcp::protocol::LegacyState::None
    );

    for (id, params) in [
        (
            1,
            json!({"capabilities": {}, "clientInfo": {"name": "x", "version": "1"}}),
        ),
        (
            2,
            json!({"protocolVersion": "2025-11-25", "clientInfo": {"name": "x", "version": "1"}}),
        ),
        (
            3,
            json!({"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "x"}}),
        ),
    ] {
        let response = dispatch(
            &mut session,
            json!({"jsonrpc": "2.0", "id": id, "method": "initialize", "params": params}),
        )
        .expect("invalid initialize response");
        validate("2025-11-25", &response);
        assert_eq!(response["error"]["code"], -32602);
        assert_eq!(
            response["error"]["message"],
            "Invalid initialize parameters"
        );
    }

    let response =
        dispatch(&mut session, legacy_initialize(4, "2025-11-25")).expect("initialize response");
    validate("2025-11-25", &response);
    assert_eq!(
        session.legacy_state(),
        cribra_mcp::protocol::LegacyState::AwaitingInitialized
    );
}

#[test]
fn dual_era_coexists_without_discovery_committing_the_session() {
    let mut session = Session::new();
    let discover = dispatch(
        &mut session,
        modern_request(1, "server/discover", json!({"_meta": meta(MODERN_VERSION)})),
    )
    .expect("discover response");
    validate(MODERN_VERSION, &discover);
    assert_eq!(
        session.legacy_state(),
        cribra_mcp::protocol::LegacyState::None
    );

    let initialize = dispatch(&mut session, legacy_initialize(2, "2025-11-25"))
        .expect("initialize after discovery");
    validate("2025-11-25", &initialize);
    assert_eq!(initialize["result"]["protocolVersion"], "2025-11-25");

    assert!(
        dispatch(
            &mut session,
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .is_none()
    );

    let modern = dispatch(&mut session, modern_meta_request(3, "ping")).expect("modern ping");
    validate(MODERN_VERSION, &modern);
    assert_eq!(modern["error"]["code"], -32601);

    let legacy = dispatch(
        &mut session,
        json!({"jsonrpc": "2.0", "id": 4, "method": "ping"}),
    )
    .expect("legacy ping");
    validate("2025-11-25", &legacy);
    assert_eq!(legacy["result"], json!({}));

    let modern_discover_after_legacy = dispatch(
        &mut session,
        modern_request(5, "server/discover", json!({"_meta": meta(MODERN_VERSION)})),
    )
    .expect("modern discover after legacy");
    validate(MODERN_VERSION, &modern_discover_after_legacy);
    assert_eq!(
        modern_discover_after_legacy["result"]["resultType"],
        "complete"
    );
}

#[test]
fn tool_contract_schemas_accept_and_reject_the_documented_shapes() {
    let tools = tooling::tools_array().expect("tool contracts parse");
    assert_eq!(tools.len(), 2);
    assert_eq!(tool_names(&json!({"tools": tools})), tooling::TOOL_NAMES);

    let scan_input = tooling::parse_schema(tooling::SCAN_INPUT_SCHEMA).expect("scan input");
    let scan_input_validator =
        jsonschema::validator_for(&scan_input).expect("scan input schema compiles");
    assert!(scan_input_validator.is_valid(&json!({"source": "hello"})));
    assert!(scan_input_validator.is_valid(&json!({
        "source": "hello",
        "rules": [{"id": "custom.rule", "pattern": "abc", "severity": "high"}]
    })));
    assert!(!scan_input_validator.is_valid(&json!({})));
    assert!(!scan_input_validator.is_valid(&json!({"source": "x", "key": "secret"})));
    assert!(!scan_input_validator.is_valid(&json!({"source": "x", "rules": "nope"})));

    let transform_input =
        tooling::parse_schema(tooling::TRANSFORM_INPUT_SCHEMA).expect("transform input");
    let transform_input_validator =
        jsonschema::validator_for(&transform_input).expect("transform input schema compiles");
    assert!(transform_input_validator.is_valid(&json!({"source": "x", "mode": "redact"})));
    assert!(transform_input_validator.is_valid(&json!({
        "source": "x",
        "mode": "template",
        "options": {"namespace": "EXAMPLE", "numbered": true}
    })));
    assert!(!transform_input_validator.is_valid(&json!({"source": "x", "mode": "synthesize"})));
    assert!(!transform_input_validator.is_valid(&json!({
        "source": "x",
        "mode": "redact",
        "key": "secret"
    })));

    let scan_output = tooling::parse_schema(tooling::SCAN_OUTPUT_SCHEMA).expect("scan output");
    let scan_output_validator =
        jsonschema::validator_for(&scan_output).expect("scan output schema compiles");
    let empty_summary = json!({
        "scannedSources": 1,
        "scannedBytes": 5,
        "reportsWithFindings": 0,
        "reportsWithoutFindings": 1,
        "totalFindings": 0,
        "totalCandidates": 0,
        "reportsWithCandidates": 0,
        "critical": 0,
        "high": 0,
        "medium": 0,
        "low": 0,
        "info": 0,
    });
    assert!(scan_output_validator.is_valid(&json!({
        "status": "ok",
        "findings": [],
        "candidates": [],
        "summary": empty_summary,
    })));
    assert!(!scan_output_validator.is_valid(&json!({"status": "ok"})));
    assert!(scan_output_validator.is_valid(&json!({
        "status": "error",
        "error": {"code": "internal_error", "message": "unavailable"}
    })));

    let transform_output =
        tooling::parse_schema(tooling::TRANSFORM_OUTPUT_SCHEMA).expect("transform output");
    let transform_output_validator =
        jsonschema::validator_for(&transform_output).expect("transform output schema compiles");
    assert!(transform_output_validator.is_valid(&json!({
        "status": "ok",
        "mode": "redact",
        "output": "hello [REDACTED]",
        "bytesIn": 20,
        "bytesOut": 16,
        "findingCount": 1,
    })));
    assert!(!transform_output_validator.is_valid(&json!({
        "status": "ok",
        "mode": "redact",
        "bytesIn": 20,
        "bytesOut": 16,
        "findingCount": 1,
    })));
    assert!(transform_output_validator.is_valid(&json!({
        "status": "error",
        "error": {"code": "internal_error", "message": "unavailable"}
    })));

    let stub = tooling::stub_tool_result();
    assert!(
        transform_output_validator.is_valid(&stub["structuredContent"]),
        "stub structuredContent must match the transform output contract"
    );
}
