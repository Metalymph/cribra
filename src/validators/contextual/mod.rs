//! Validators that require source context in addition to the candidate value.

mod context;

pub(crate) mod aws;
pub(crate) mod azure;
pub(crate) mod gcp;
pub(crate) mod generic;
pub(crate) mod hash;
pub(crate) mod password;
pub(crate) mod utils;

pub(crate) use context::ValidationContext;
