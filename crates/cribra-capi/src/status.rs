//! Stable-sized status values returned by the native ABI.

/// Native ABI status representation.
///
/// Numeric assignments become part of the C ABI contract once released.
pub type CribraStatus = u32;

/// Operation completed successfully.
pub const CRIBRA_OK: CribraStatus = 0;
/// A required pointer or other argument was invalid.
pub const CRIBRA_INVALID_ARGUMENT: CribraStatus = 1;
/// A length-delimited input buffer was not valid UTF-8.
pub const CRIBRA_INVALID_UTF8: CribraStatus = 2;
/// A custom rule definition could not be accepted.
pub const CRIBRA_RULE_ERROR: CribraStatus = 3;
/// Scanner construction failed.
pub const CRIBRA_BUILD_ERROR: CribraStatus = 4;
/// A source transformation could not be applied safely.
pub const CRIBRA_TRANSFORM_ERROR: CribraStatus = 5;
/// An index was outside the bounds of the requested report collection.
pub const CRIBRA_OUT_OF_RANGE: CribraStatus = 6;
/// Core rejected a ruleset that cannot be streamed exactly.
pub const CRIBRA_STREAM_UNSUPPORTED_RULE_SET: CribraStatus = 7;
/// The stream has finished or a terminal failure discarded its session.
pub const CRIBRA_STREAM_NOT_ACTIVE: CribraStatus = 8;
/// A streaming chunk contains malformed UTF-8; the session is terminal.
pub const CRIBRA_STREAM_INVALID_UTF8: CribraStatus = 9;
/// Streaming EOF occurred inside an incomplete UTF-8 scalar.
pub const CRIBRA_STREAM_INCOMPLETE_UTF8: CribraStatus = 10;
/// Core's cumulative raw rule candidate limit was exceeded.
pub const CRIBRA_STREAM_CANDIDATE_LIMIT_EXCEEDED: CribraStatus = 11;
/// Core's retained finding/review result limit was exceeded.
pub const CRIBRA_STREAM_RESULT_LIMIT_EXCEEDED: CribraStatus = 12;
/// Core's logical streaming storage admission limit was exceeded.
pub const CRIBRA_STREAM_STORAGE_LIMIT_EXCEEDED: CribraStatus = 13;
/// Core's streaming storage requirement arithmetic overflowed.
pub const CRIBRA_STREAM_STORAGE_REQUIREMENT_OVERFLOW: CribraStatus = 14;
/// Core or adapter source-byte accounting overflowed.
pub const CRIBRA_STREAM_SOURCE_OFFSET_OVERFLOW: CribraStatus = 15;
/// Core's source line or Unicode-scalar column arithmetic overflowed.
pub const CRIBRA_STREAM_COORDINATE_OVERFLOW: CribraStatus = 16;
/// Core's streaming execution invariant failed.
pub const CRIBRA_STREAM_EXECUTION_INVARIANT_VIOLATED: CribraStatus = 17;
/// A future Core streaming failure is not understood by this adapter.
pub const CRIBRA_STREAM_UNKNOWN_ERROR: CribraStatus = 18;
/// A previous terminal Core failure prevents further incremental edits.
pub const CRIBRA_INCREMENTAL_NOT_ACTIVE: CribraStatus = 19;
/// The edit's expected revision differs from the committed revision.
pub const CRIBRA_INCREMENTAL_STALE_REVISION: CribraStatus = 20;
/// The edit range is reversed or outside the document.
pub const CRIBRA_INCREMENTAL_INVALID_RANGE: CribraStatus = 21;
/// An edit endpoint divides a UTF-8 scalar.
pub const CRIBRA_INCREMENTAL_INVALID_UTF8_BOUNDARY: CribraStatus = 22;
/// The committed revision cannot be incremented; the error is recoverable.
pub const CRIBRA_INCREMENTAL_REVISION_OVERFLOW: CribraStatus = 23;
/// The replacement document length is not representable; the error is recoverable.
pub const CRIBRA_INCREMENTAL_DOCUMENT_LENGTH_OVERFLOW: CribraStatus = 24;
/// Core's per-operation raw candidate limit was exceeded.
pub const CRIBRA_INCREMENTAL_CANDIDATE_LIMIT_EXCEEDED: CribraStatus = 25;
/// Core's simultaneous old and staged cache record limit was exceeded.
pub const CRIBRA_INCREMENTAL_CACHE_LIMIT_EXCEEDED: CribraStatus = 26;
/// Core's per-operation result materialization limit was exceeded.
pub const CRIBRA_INCREMENTAL_RESULT_LIMIT_EXCEEDED: CribraStatus = 27;
/// Core's logical transaction payload limit was exceeded.
pub const CRIBRA_INCREMENTAL_TRANSACTION_STORAGE_LIMIT_EXCEEDED: CribraStatus = 28;
/// Core's resource accounting arithmetic overflowed.
pub const CRIBRA_INCREMENTAL_RESOURCE_ACCOUNTING_OVERFLOW: CribraStatus = 29;
/// Core's incremental execution invariant failed.
pub const CRIBRA_INCREMENTAL_EXECUTION_INVARIANT_VIOLATED: CribraStatus = 30;
/// A future Core incremental failure is not understood by this adapter.
pub const CRIBRA_INCREMENTAL_UNKNOWN_ERROR: CribraStatus = 31;
/// A contained panic permanently invalidated the adapter's Core session.
pub const CRIBRA_INCREMENTAL_SESSION_INVALIDATED: CribraStatus = 32;
/// An unexpected panic was contained inside the native adapter.
pub const CRIBRA_INTERNAL_ERROR: CribraStatus = 255;
