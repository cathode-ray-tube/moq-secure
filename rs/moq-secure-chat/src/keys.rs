use ed25519_dalek::{SigningKey, VerifyingKey};
use moq_secure::MoqSecureError;
use rand::rngs::OsRng;
use rand::RngCore;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatKeysError {
    #[error("key_id must be 0..=255 (got {0})")]
    KeyIdOutOfRange(u16),

    #[error("aead key must be 32 bytes (decoded {0} bytes)")]
    AeadKeyWrongLen(usize),

    #[error("failed to decode key string as hex or base64: {0}")]
    DecodeFailed(String),
}

fn decode_hex_or_b64(s: &str) -> Result<Vec<u8>, ChatKeysError> {
    let t = s.trim();

    // Prefer hex if it looks like hex.
    let is_hex = t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex {
        hex::decode(t).map_err(|e| ChatKeysError::DecodeFailed(e.to_string()))
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(t)
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(t))
            .map_err(|e| ChatKeysError::DecodeFailed(e.to_string()))
    }
}

#[derive(Debug, Clone)]
pub struct ChatKeys {
    pub key_id: u8,
    pub aead_key: [u8; 32],

    // broadcaster signature keypair
    pub signing_private: SigningKey,
    pub signing_verify: VerifyingKey,
}

impl ChatKeys {
    pub fn generate(key_id: Option<u8>) -> Result<Self, ChatKeysError> {
        let mut aead_key = [0u8; 32];
        OsRng.fill_bytes(&mut aead_key);

        let key_id = key_id.unwrap_or_else(|| (OsRng.next_u32() % 256) as u8);

        let signing_private = SigningKey::generate(&mut OsRng);
        let signing_verify = signing_private.verifying_key();

        Ok(Self {
            key_id,
            aead_key,
            signing_private,
            signing_verify,
        })
    }

    pub fn from_aead_and_signing(
        key_id: u8,
        aead_key: [u8; 32],
        signing_private: SigningKey,
    ) -> Self {
        let signing_verify = signing_private.verifying_key();
        Self {
            key_id,
            aead_key,
            signing_private,
            signing_verify,
        }
    }

    pub fn aead_key_hex(&self) -> String {
        hex::encode(self.aead_key)
    }

    pub fn signing_verify_hex(&self) -> String {
        hex::encode(self.signing_verify.to_bytes())
    }

    pub fn signing_private_hex_seed(&self) -> String {
        // ed25519-dalek SigningKey::to_bytes() returns 32-byte seed.
        hex::encode(self.signing_private.to_bytes())
    }

    pub fn from_strings(
        key_id: u8,
        aead_key_str: &str,
        signing_private_seed_or_bytes_str: &str,
    ) -> Result<Self, ChatKeysError> {
        let aead_decoded = decode_hex_or_b64(aead_key_str)?;
        if aead_decoded.len() != 32 {
            return Err(ChatKeysError::AeadKeyWrongLen(aead_decoded.len()));
        }
        let aead_key: [u8; 32] = aead_decoded
            .try_into()
            .map_err(|_| ChatKeysError::AeadKeyWrongLen(aead_decoded.len()))?;

        let seed_decoded = decode_hex_or_b64(signing_private_seed_or_bytes_str)?;
        // We support either 32-byte seed (preferred) or 64-byte private key bytes.
        let signing_private = if seed_decoded.len() == 32 {
            SigningKey::from_bytes(&seed_decoded.into())
        } else if seed_decoded.len() == 64 {
            // If you provide 64 bytes, convert to SigningKey seed (ed25519-dalek supports
            // from_bytes as seed, so we reject 64-byte input for determinism).
            return Err(ChatKeysError::DecodeFailed(
                "Provided signing key decoded to 64 bytes; expected 32-byte seed hex/base64".into(),
            ));
        } else {
            return Err(ChatKeysError::DecodeFailed(format!(
                "Provided signing key decoded to {} bytes; expected 32-byte seed",
                seed_decoded.len()
            )));
        };

        Ok(Self {
            key_id,
            aead_key,
            signing_private,
            signing_verify: signing_private.verifying_key(),
        })
    }

    pub fn verify_from_hex(verify_hex: &str) -> Result<ed25519_dalek::VerifyingKey, ChatKeysError> {
        let bytes = hex::decode(verify_hex).map_err(|e| ChatKeysError::DecodeFailed(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(ChatKeysError::DecodeFailed(format!(
                "verify key decoded to {} bytes; expected 32",
                bytes.len()
            )));
        }
        let arr: [u8; 32] = bytes.try_into().unwrap();
        Ok(ed25519_dalek::VerifyingKey::from_bytes(&arr).unwrap())
    }
}
