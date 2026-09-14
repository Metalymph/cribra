//! Validators that require source context in addition to the candidate value.

mod context;

pub(crate) mod aws;
pub(crate) mod azure;
pub(crate) mod cargo_registry;
pub(crate) mod database_connection;
pub(crate) mod docker_registry;
pub(crate) mod gcp;
pub(crate) mod generic;
pub(crate) mod hash;
pub(crate) mod http_basic;
pub(crate) mod netrc;
pub(crate) mod npm_registry;
pub(crate) mod nuget;
pub(crate) mod password;
pub(crate) mod pypi;
pub(crate) mod rubygems;
pub(crate) mod system_password_verifier;
pub(crate) mod utils;
pub(crate) mod wireguard;

pub(crate) use context::ValidationContext;
