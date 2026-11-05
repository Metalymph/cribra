//! Typed, source-free transformation failures.
//!
//! These error objects carry a stable numeric code so TypeScript callers can
//! normalize failures without parsing diagnostic text. Messages never contain
//! source text, matched values, replacements, or key material.

use wasm_bindgen::prelude::*;

/// Stable transformation failure codes. Existing assignments never move.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformErrorCode {
    /// A finding span is empty, reversed, or extends outside the source.
    InvalidSpan = 0,
    /// A finding span endpoint divides a UTF-8 scalar.
    InvalidUtf8Boundary = 1,
    /// Two findings overlap in a transformation that requires independent spans.
    OverlappingSpans = 2,
    /// The supplied key does not contain exactly 32 bytes.
    InvalidKeyLength = 3,
    /// A future Core transformation failure is not understood by this adapter.
    UnknownCoreError = 4,
}

/// Owned thrown wasm-bindgen object. Inspect it and call generated free() once.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransformError {
    code: TransformErrorCode,
}

#[wasm_bindgen]
impl TransformError {
    /// Stable reason independent of diagnostic wording.
    #[wasm_bindgen(getter)]
    pub fn code(&self) -> TransformErrorCode {
        self.code
    }

    /// Fixed diagnostics contain neither source nor transformation material.
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        match self.code {
            TransformErrorCode::InvalidSpan => "finding span is invalid for the source",
            TransformErrorCode::InvalidUtf8Boundary => {
                "finding span is not aligned to UTF-8 character boundaries"
            }
            TransformErrorCode::OverlappingSpans => {
                "transformation requires non-overlapping findings"
            }
            TransformErrorCode::InvalidKeyLength => {
                "transformation key must contain exactly 32 bytes"
            }
            TransformErrorCode::UnknownCoreError => "unknown Core transformation failure",
        }
        .to_owned()
    }
}

impl TransformError {
    /// Builds one typed failure from an adapter-owned classification.
    pub(crate) const fn new(code: TransformErrorCode) -> Self {
        Self { code }
    }
}

impl From<cribra::transform::TransformError> for TransformError {
    fn from(error: cribra::transform::TransformError) -> Self {
        use cribra::transform::TransformError as Core;
        let code = match error {
            Core::InvalidSpan { .. } => TransformErrorCode::InvalidSpan,
            Core::InvalidUtf8Boundary { .. } => TransformErrorCode::InvalidUtf8Boundary,
            Core::OverlappingSpans { .. } => TransformErrorCode::OverlappingSpans,
            _ => TransformErrorCode::UnknownCoreError,
        };
        Self { code }
    }
}
