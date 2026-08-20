//! Stable-sized status values returned by the native ABI.

/// Native ABI status representation.
pub type CribraStatus = u32;

/// Operation completed successfully.
pub const CRIBRA_OK: CribraStatus = 0;
/// A required pointer or other argument was invalid.
pub const CRIBRA_INVALID_ARGUMENT: CribraStatus = 1;
/// A length-delimited input buffer was not valid UTF-8.
pub const CRIBRA_INVALID_UTF8: CribraStatus = 2;
/// Scanner construction failed.
pub const CRIBRA_BUILD_ERROR: CribraStatus = 4;
/// An unexpected panic was contained inside the native adapter.
pub const CRIBRA_INTERNAL_ERROR: CribraStatus = 255;
