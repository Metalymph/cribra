//! Hard limits shared by the transport and protocol layers.

/// Maximum size of one newline-delimited JSON-RPC message, excluding the
/// trailing newline.
pub const MAX_MESSAGE_BYTES: usize = 2 * 1024 * 1024;

/// Maximum UTF-8 byte length of the `source` tool argument.
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// Maximum size of one fully serialized outgoing JSON-RPC message, including
/// every MCP content representation.
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Maximum number of custom regular-expression rules per tool call.
pub const MAX_CUSTOM_RULES: usize = 32;

/// Maximum UTF-8 byte length of a custom rule identifier.
pub const MAX_CUSTOM_RULE_ID_BYTES: usize = 128;

/// Maximum number of Unicode scalar values in a custom rule pattern.
pub const MAX_CUSTOM_PATTERN_CHARS: usize = 4096;

/// Maximum number of findings returned by one tool call.
pub const MAX_FINDINGS: usize = 4096;

/// Maximum number of review candidates returned by one tool call.
pub const MAX_CANDIDATES: usize = 4096;

/// Maximum number of Unicode scalar values in a redaction replacement.
pub const MAX_REDACTION_CHARS: usize = 256;

/// Maximum number of Unicode scalar values in a template namespace.
pub const MAX_TEMPLATE_NAMESPACE_CHARS: usize = 64;

/// Maximum number of accepted requests waiting for the worker.
pub const REQUEST_QUEUE_CAPACITY: usize = 8;

/// Maximum number of responses waiting for the writer.
pub const WRITER_QUEUE_CAPACITY: usize = 8;

/// Maximum number of tracked request identifiers.
pub const CANCELLATION_REGISTRY_CAPACITY: usize = 1024;

/// Maximum request identifier length in UTF-8 bytes.
pub const MAX_REQUEST_ID_BYTES: usize = 128;

/// Size of the transport read buffer.
pub const READ_CHUNK_BYTES: usize = 8 * 1024;

/// Exit code used when sustained writer backpressure makes the connection
/// terminally unhealthy.
pub const EXIT_BACKPRESSURE: i32 = 3;

/// Default no-progress deadline for stdout writes.
pub const DEFAULT_BACKPRESSURE_TIMEOUT_MS: u64 = 5_000;

/// Environment variable used by host tests to shorten the terminal
/// backpressure deadline. Production behavior uses the default.
pub const BACKPRESSURE_TIMEOUT_ENV: &str = "CRIBRA_MCP_BACKPRESSURE_TIMEOUT_MS";

/// Exit code used for startup/configuration failures.
pub const EXIT_CONFIG: i32 = 2;

/// Exit code used for internal transport failures.
pub const EXIT_INTERNAL: i32 = 1;
