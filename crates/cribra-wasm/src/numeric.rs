//! Shared lossless JavaScript number validation before target-width conversion.
use wasm_bindgen::{JsError, JsValue};

pub(crate) fn checked_usize(value: f64) -> Option<usize> {
    const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
    if !value.is_finite()
        || value < 0.0
        || value.fract() != 0.0
        || value > MAX_SAFE_INTEGER
        || value > usize::MAX as f64
    {
        return None;
    }
    Some(value as usize)
}

pub(crate) fn js_usize(value: &JsValue) -> Option<usize> {
    checked_usize(value.as_f64()?)
}

pub(crate) fn js_index(value: &JsValue) -> Result<usize, JsError> {
    js_usize(value).ok_or_else(|| {
        JsError::new("index must be a nonnegative safe integer representable on this target")
    })
}

pub(crate) fn js_limit(value: JsValue) -> Result<Option<usize>, ()> {
    if value.is_undefined() {
        Ok(None)
    } else {
        js_usize(&value).map(Some).ok_or(())
    }
}
