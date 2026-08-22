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
/// An unexpected panic was contained inside the native adapter.
pub const CRIBRA_INTERNAL_ERROR: CribraStatus = 255;
