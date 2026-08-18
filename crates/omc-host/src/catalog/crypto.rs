//! Catalog authentication using the standard HMAC implementation.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid HMAC key: {0}")]
    InvalidKey(String),
}

pub fn keyed_signature(key: &str, digest: &str) -> Result<String, CryptoError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes())
        .map_err(|error| CryptoError::InvalidKey(error.to_string()))?;
    mac.update(digest.as_bytes());
    Ok(mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(super) fn signature_matches(key: &str, digest: &str, signature: &str) -> bool {
    let Some(tag) = decode_hex(signature) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(key.as_bytes()) else {
        return false;
    };
    mac.update(digest.as_bytes());
    mac.verify_slice(&tag).is_ok()
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect::<Option<Vec<_>>>()
        .filter(|_| value.len().is_multiple_of(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_is_exact_and_rejects_malformed_tags() {
        let signature = keyed_signature("key", "digest").unwrap();
        assert!(signature_matches("key", "digest", &signature));
        assert!(!signature_matches("key", "other", &signature));
        assert!(!signature_matches("key", "digest", "not-hex"));
    }
}
