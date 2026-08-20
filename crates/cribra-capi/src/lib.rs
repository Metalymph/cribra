//! Native C ABI adapter for Cribra.
//!
//! This crate is the dedicated native interoperability boundary around the
//! Rust-native `cribra` core. The core remains free of FFI concerns and keeps
//! its `#![forbid(unsafe_code)]` invariant.
//!
//! v0.3.2 establishes only the crate and native artifact boundary. Public C ABI
//! entry points are introduced by later roadmap steps.

#![warn(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
