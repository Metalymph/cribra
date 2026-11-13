//! Cribra's Model Context Protocol (MCP) stdio foundation.
//!
//! M1 implements the modern `2026-07-28` stateless protocol and the legacy
//! `2025-11-25` lifecycle over newline-delimited JSON-RPC. It advertises two
//! static tool contracts whose handlers are deliberate stubs; it does not claim
//! functional scan or transform interoperability.

#![cfg_attr(not(unix), allow(unused))]

#[cfg(not(unix))]
compile_error!("cribra-mcp currently supports macOS and Linux stdio transports only");

pub mod error;
pub mod jsonrpc;
pub mod limits;
pub mod protocol;
pub mod registry;
pub mod server;
pub mod tooling;
pub mod transport;
