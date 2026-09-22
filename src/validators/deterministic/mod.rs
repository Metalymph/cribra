//! Validators for credentials whose structure can be determined from the
//! candidate value alone.

pub(crate) mod cloudflare;
pub(crate) mod codice_fiscale;
pub(crate) mod github;
pub(crate) mod gitlab;
pub(crate) mod iban;
pub(crate) mod jwt;
pub(crate) mod pesel;
pub(super) mod rubygems;
pub(crate) mod slack;
pub(crate) mod stripe;
pub(crate) mod telegram;
