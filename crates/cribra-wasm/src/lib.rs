//! WebAssembly interoperability adapter for Cribra.
//!
//! This crate projects the authoritative Rust-native `cribra` core into a
//! JavaScript-friendly WebAssembly API. It contains no DOM, Worker, networking,
//! storage, or application-specific policy logic.
//!
//! Browser consumers own source lifecycle and isolation. Source text crosses
//! the WASM boundary only for explicit operations such as scanning or
//! transformation.

use cribra::Scanner;
use wasm_bindgen::prelude::*;

/// Browser-facing scanner backed by Cribra's authoritative built-in catalog.
#[wasm_bindgen]
pub struct ScanEngine {
    scanner: Scanner,
}

#[wasm_bindgen]
impl ScanEngine {
    /// Compiles the current authoritative Cribra rule catalog.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            scanner: Scanner::default(),
        }
    }

    /// Returns the number of compiled rules in this engine.
    #[wasm_bindgen(js_name = rulesCount)]
    pub fn rules_count(&self) -> usize {
        self.scanner.rules_count()
    }
}

impl Default for ScanEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_engine_uses_current_builtins() {
        let engine = ScanEngine::new();

        assert_eq!(engine.rules_count(), cribra::builtins::CURRENT.len());
    }
}
