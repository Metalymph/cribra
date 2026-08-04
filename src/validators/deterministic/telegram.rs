//! Structural validation for Telegram Bot API tokens.

use crate::validators::utils::{has_ascii_len, is_ascii_decimal, is_obvious_placeholder};

const MIN_BOT_ID_LEN: usize = 5;
const MAX_BOT_ID_LEN: usize = 20;
const MIN_SECRET_LEN: usize = 20;
const MAX_SECRET_LEN: usize = 128;
const MAX_TOKEN_LEN: usize = MAX_BOT_ID_LEN + 1 + MAX_SECRET_LEN;

/// Successful Telegram bot-token structural validation.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct TelegramValidation;

/// Validates a possible Telegram Bot API token.
pub(crate) fn validate_telegram_bot_token(candidate: &str) -> Option<TelegramValidation> {
    if !has_ascii_len(candidate, 1, MAX_TOKEN_LEN) || is_obvious_placeholder(candidate) {
        return None;
    }

    let (bot_id, secret) = candidate.split_once(':')?;

    if !(MIN_BOT_ID_LEN..=MAX_BOT_ID_LEN).contains(&bot_id.len())
        || !is_ascii_decimal(bot_id)
        || !(MIN_SECRET_LEN..=MAX_SECRET_LEN).contains(&secret.len())
        || is_obvious_placeholder(secret)
        || !secret.bytes().all(is_telegram_secret_byte)
    {
        return None;
    }

    Some(TelegramValidation)
}

#[inline]
const fn is_telegram_secret_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_documented_shape() {
        assert!(validate_telegram_bot_token("123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11").is_some());
    }

    #[test]
    fn rejects_invalid_shapes() {
        assert!(validate_telegram_bot_token("bot123:ABCDEF1234567890_abcdef-1234567890").is_none());
        assert!(validate_telegram_bot_token("123456:short").is_none());
        assert!(validate_telegram_bot_token("123456:ABCDEF1234567890_abcdef+invalid").is_none());
    }
}
