//! MCP protocol negotiation and request dispatch.
//!
//! The modern revision is stateless: each request carries its version and
//! capabilities in `_meta`. The legacy revision is process-scoped and starts
//! with an `initialize` request. Both eras may be served by one process; a
//! modern discovery request never commits the process to modern-only behavior.

use serde_json::{Value, json};

use crate::error::{
    INITIALIZE_ALREADY_COMPLETED, INTERNAL_ERROR, INVALID_INITIALIZE, INVALID_TOOL_CALL,
    METHOD_NOT_FOUND, MISSING_META, NOT_INITIALIZED, RpcError, UNKNOWN_TOOL,
    unsupported_protocol_version,
};
use crate::jsonrpc::{ParsedMessage, RequestId};
use crate::tooling;

/// The modern MCP protocol revision implemented by this crate.
pub const MODERN_VERSION: &str = "2026-07-28";
/// The legacy MCP protocol revision implemented by this crate.
pub const LEGACY_VERSION: &str = "2025-11-25";
/// Server implementation name advertised in results.
pub const SERVER_NAME: &str = "cribra-mcp";
/// Server implementation version advertised in results.
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

const PROTOCOL_VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";
const CLIENT_CAPABILITIES_KEY: &str = "io.modelcontextprotocol/clientCapabilities";
const CLIENT_INFO_KEY: &str = "io.modelcontextprotocol/clientInfo";
const SERVER_INFO_KEY: &str = "io.modelcontextprotocol/serverInfo";

/// The wire era used to shape one response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Era {
    /// Per-request metadata, `resultType`, and caching hints.
    Modern,
    /// Process-scoped lifecycle without `resultType` or caching hints.
    Legacy,
}

/// Legacy lifecycle state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LegacyState {
    /// No legacy `initialize` has been accepted.
    #[default]
    None,
    /// `initialize` succeeded and `notifications/initialized` is pending.
    AwaitingInitialized,
    /// The legacy lifecycle is active.
    Active,
}

/// A tool invocation accepted for worker execution.
#[derive(Clone, Debug)]
pub struct ToolCall {
    /// Request identifier used for cancellation and duplicate suppression.
    pub id: RequestId,
    /// The era whose response shape must be used.
    pub era: Era,
    /// Tool name, already checked against the static contract.
    pub name: String,
    /// Tool arguments, already checked to be an object.
    pub arguments: Value,
}

/// The action required by one parsed request.
#[derive(Clone, Debug)]
pub enum Dispatch {
    /// Respond immediately.
    Reply(Reply),
    /// Hand the request to the worker and respond later.
    ToolCall(ToolCall),
    /// No response is required.
    None,
}

/// A complete JSON-RPC response payload.
#[derive(Clone, Debug)]
pub struct Reply {
    /// The request identifier, or `None` for transport-level errors.
    pub id: Option<RequestId>,
    /// The success or fixed error outcome.
    pub outcome: ReplyOutcome,
}

/// A JSON-RPC success result or fixed error.
#[derive(Clone, Debug)]
pub enum ReplyOutcome {
    /// A success result, already shaped for its era.
    Success(Value),
    /// A protocol error.
    Error(RpcError),
}

impl Reply {
    /// Creates a success reply.
    #[must_use]
    pub fn success(id: RequestId, mut result: Value, era: Era) -> Self {
        shape_success(era, &mut result);
        Self {
            id: Some(id),
            outcome: ReplyOutcome::Success(result),
        }
    }

    /// Creates an error reply.
    #[must_use]
    pub fn error(id: Option<RequestId>, error: RpcError) -> Self {
        Self {
            id,
            outcome: ReplyOutcome::Error(error),
        }
    }

    /// Serializes this reply as a JSON-RPC response value.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match &self.outcome {
            ReplyOutcome::Success(result) => json!({
                "jsonrpc": "2.0",
                "id": self.id.as_ref().map_or(Value::Null, RequestId::to_value),
                "result": result,
            }),
            ReplyOutcome::Error(error) => crate::error::error_value(self.id.as_ref(), error),
        }
    }
}

/// Applies modern-only result fields. Legacy results remain unchanged.
pub fn shape_success(era: Era, result: &mut Value) {
    if era == Era::Legacy {
        return;
    }
    if !result.is_object() {
        *result = json!({});
    }
    if let Value::Object(object) = result {
        object.insert("resultType".to_owned(), Value::from("complete"));
        object.insert(
            "_meta".to_owned(),
            json!({
                SERVER_INFO_KEY: server_info(),
            }),
        );
    }
}

/// The server implementation object used in results.
#[must_use]
pub fn server_info() -> Value {
    json!({
        "name": SERVER_NAME,
        "version": SERVER_VERSION,
    })
}

/// One protocol session.
///
/// A session owns the legacy lifecycle state. It is driven from the reader
/// thread so lifecycle transitions are ordered before later requests are
/// classified.
#[derive(Debug, Default)]
pub struct Session {
    legacy: LegacyState,
}

impl Session {
    /// Creates a session with no legacy lifecycle.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            legacy: LegacyState::None,
        }
    }

    /// Returns the current legacy lifecycle state.
    #[must_use]
    pub const fn legacy_state(&self) -> LegacyState {
        self.legacy
    }

    /// Applies a notification. Cancellation is handled by the transport layer
    /// before notifications reach this method.
    pub fn handle_notification(&mut self, method: &str, _params: Option<&Value>) {
        if method == "notifications/initialized" && self.legacy == LegacyState::AwaitingInitialized
        {
            self.legacy = LegacyState::Active;
        }
    }

    /// Dispatches one parsed request.
    #[must_use]
    pub fn dispatch(&mut self, parsed: ParsedMessage) -> Dispatch {
        match parsed {
            ParsedMessage::Notification { method, params } => {
                self.handle_notification(&method, params.as_ref());
                Dispatch::None
            }
            ParsedMessage::Invalid { id, error } => Dispatch::Reply(Reply::error(id, error)),
            ParsedMessage::Request { id, method, params } => {
                self.dispatch_request(id, &method, params)
            }
        }
    }

    fn dispatch_request(&mut self, id: RequestId, method: &str, params: Option<Value>) -> Dispatch {
        let has_modern_meta = has_modern_protocol_meta(params.as_ref());

        if method == "initialize" && !has_modern_meta {
            return self.legacy_initialize(id, params);
        }
        if has_modern_meta {
            return self.modern_request(id, method, params);
        }
        match self.legacy {
            LegacyState::Active | LegacyState::AwaitingInitialized => {
                self.legacy_request(id, method, params)
            }
            LegacyState::None => Dispatch::Reply(Reply::error(Some(id), MISSING_META)),
        }
    }

    fn modern_request(&mut self, id: RequestId, method: &str, params: Option<Value>) -> Dispatch {
        if let Err(error) = validate_modern_meta(params.as_ref()) {
            return Dispatch::Reply(Reply::error(Some(id), error));
        }

        match method {
            "server/discover" => Dispatch::Reply(Reply::success(
                id,
                json!({
                    "supportedVersions": [MODERN_VERSION],
                    "capabilities": {"tools": {}},
                    "ttlMs": 0,
                    "cacheScope": "private",
                }),
                Era::Modern,
            )),
            "tools/list" => match tooling::tools_array() {
                Ok(tools) => Dispatch::Reply(Reply::success(
                    id,
                    json!({
                        "tools": tools,
                        "ttlMs": 0,
                        "cacheScope": "private",
                    }),
                    Era::Modern,
                )),
                Err(_) => Dispatch::Reply(Reply::error(Some(id), INTERNAL_ERROR)),
            },
            "tools/call" => self.tool_call(id, Era::Modern, params),
            _ => Dispatch::Reply(Reply::error(Some(id), METHOD_NOT_FOUND)),
        }
    }

    fn legacy_initialize(&mut self, id: RequestId, params: Option<Value>) -> Dispatch {
        if self.legacy != LegacyState::None {
            return Dispatch::Reply(Reply::error(Some(id), INITIALIZE_ALREADY_COMPLETED));
        }
        if validate_legacy_initialize(params.as_ref()).is_err() {
            return Dispatch::Reply(Reply::error(Some(id), INVALID_INITIALIZE));
        }
        self.legacy = LegacyState::AwaitingInitialized;
        Dispatch::Reply(Reply::success(
            id,
            json!({
                "protocolVersion": LEGACY_VERSION,
                "capabilities": {"tools": {}},
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": SERVER_VERSION,
                },
            }),
            Era::Legacy,
        ))
    }

    fn legacy_request(&mut self, id: RequestId, method: &str, params: Option<Value>) -> Dispatch {
        if method == "initialize" {
            return Dispatch::Reply(Reply::error(Some(id), INITIALIZE_ALREADY_COMPLETED));
        }

        // A client should wait for the initialize response and send
        // `notifications/initialized`; pings remain harmless in either state.
        if method == "ping" {
            return Dispatch::Reply(Reply::success(id, json!({}), Era::Legacy));
        }
        if self.legacy != LegacyState::Active {
            return Dispatch::Reply(Reply::error(Some(id), NOT_INITIALIZED));
        }

        match method {
            "tools/list" => match tooling::tools_array() {
                Ok(tools) => {
                    Dispatch::Reply(Reply::success(id, json!({"tools": tools}), Era::Legacy))
                }
                Err(_) => Dispatch::Reply(Reply::error(Some(id), INTERNAL_ERROR)),
            },
            "tools/call" => self.tool_call(id, Era::Legacy, params),
            _ => Dispatch::Reply(Reply::error(Some(id), METHOD_NOT_FOUND)),
        }
    }

    fn tool_call(&self, id: RequestId, era: Era, params: Option<Value>) -> Dispatch {
        let Some(params) = params.as_ref().and_then(Value::as_object) else {
            return Dispatch::Reply(Reply::error(Some(id), INVALID_TOOL_CALL));
        };
        if params.contains_key("inputResponses") || params.contains_key("requestState") {
            return Dispatch::Reply(Reply::error(Some(id), INVALID_TOOL_CALL));
        }
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Dispatch::Reply(Reply::error(Some(id), INVALID_TOOL_CALL));
        };
        let arguments = match params.get("arguments") {
            None => json!({}),
            Some(value) if value.is_object() => value.clone(),
            Some(_) => {
                return Dispatch::Reply(Reply::error(Some(id), INVALID_TOOL_CALL));
            }
        };
        if !tooling::is_known_tool(name) {
            return Dispatch::Reply(Reply::error(Some(id), UNKNOWN_TOOL));
        }
        Dispatch::ToolCall(ToolCall {
            id,
            era,
            name: name.to_owned(),
            arguments,
        })
    }
}

fn has_modern_protocol_meta(params: Option<&Value>) -> bool {
    params
        .and_then(Value::as_object)
        .and_then(|params| params.get("_meta"))
        .and_then(Value::as_object)
        .is_some_and(|meta| meta.contains_key(PROTOCOL_VERSION_KEY))
}

fn validate_modern_meta(params: Option<&Value>) -> Result<(), RpcError> {
    let Some(params) = params.and_then(Value::as_object) else {
        return Err(MISSING_META);
    };
    let Some(meta) = params.get("_meta").and_then(Value::as_object) else {
        return Err(MISSING_META);
    };
    let Some(version) = meta.get(PROTOCOL_VERSION_KEY).and_then(Value::as_str) else {
        return Err(MISSING_META);
    };
    let Some(capabilities) = meta.get(CLIENT_CAPABILITIES_KEY) else {
        return Err(MISSING_META);
    };
    if !capabilities.is_object() {
        return Err(MISSING_META);
    }
    if let Some(client_info) = meta.get(CLIENT_INFO_KEY) {
        validate_implementation(client_info).map_err(|()| MISSING_META)?;
    }
    if version != MODERN_VERSION {
        return Err(unsupported_protocol_version(version));
    }
    Ok(())
}

fn validate_legacy_initialize(params: Option<&Value>) -> Result<(), ()> {
    let Some(params) = params.and_then(Value::as_object) else {
        return Err(());
    };
    if let Some(meta) = params.get("_meta")
        && !meta.is_object()
    {
        return Err(());
    }
    if !params.get("protocolVersion").is_some_and(Value::is_string) {
        return Err(());
    }
    if !params.get("capabilities").is_some_and(Value::is_object) {
        return Err(());
    }
    let Some(client_info) = params.get("clientInfo") else {
        return Err(());
    };
    validate_implementation(client_info)
}

fn validate_implementation(value: &Value) -> Result<(), ()> {
    let Some(object) = value.as_object() else {
        return Err(());
    };
    if !object.get("name").is_some_and(Value::is_string) {
        return Err(());
    }
    if !object.get("version").is_some_and(Value::is_string) {
        return Err(());
    }
    Ok(())
}
