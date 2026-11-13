//! Hard limits shared by the transport and protocol layers.

/// Maximum size of one newline-delimited JSON-RPC message, excluding the
/// trailing newline.
pub const MAX_MESSAGE_BYTES: usize = 2 * 1024 * 1024;

/// Maximum size of one fully serialized outgoing JSON-RPC message, including
/// every MCP content representation.
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

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
