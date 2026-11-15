//! Cribra's Model Context Protocol (MCP) stdio server.
//!
//! M2 implements the modern `2026-07-28` stateless protocol and the legacy
//! `2025-11-25` lifecycle over newline-delimited JSON-RPC. It advertises two
//! functional tools backed directly by Cribra Core: `cribra_scan` returns
//! privacy-safe finding metadata, and `cribra_transform` returns Core-produced
//! redacted or templated source. Keyed transformations are not advertised.

#![cfg_attr(not(unix), allow(unused))]

#[cfg(not(unix))]
compile_error!("cribra-mcp currently supports macOS and Linux stdio transports only");

pub mod core_adapter;
pub mod error;
pub mod jsonrpc;
pub mod limits;
pub mod protocol;
pub mod registry;
pub mod server;
pub mod tool_arguments;
pub mod tooling;
pub mod transport;
