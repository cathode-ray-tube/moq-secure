use base64::Engine;
use ed25519_dalek::{SigningKey, VerifyingKey};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatKeysError {
    #[error("key_id must be 0..=255 (got {0})")]
    KeyIdOutOfRange(u16),

    #[error("aead key must be 32 bytes (decoded {0} bytes)")]
    AeadKeyWrongLen(usize),

    #[error("failed to decode key string as hex or base64: {0}")]
    DecodeFailed(String),

    #[error("signing private key must decode to either 32-byte seed or 64-byte private key bytes; got {decoded_len} bytes")]
    SigningKeyWrongLen { decoded_len: usize },

    #[error("invalid ed25519 signing/verification key: {0}")]
    SigningKeyInvalid(String),
}

fn decode_hex_or_b64(s: &str) -> Result<Vec<u8>, ChatKeysError> {
    let t = s.trim();

    if let Ok(bytes) = hex::decode(t) {
        return Ok(bytes);
    }

    base64::engine::general_purpose::STANDARD
        .decode(t)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(t))
        .map_err(|e| ChatKeysError::DecodeFailed(e.to_string()))
}

fn decode_signing_key_seed_or_keypair(bytes: Vec<u8>) -> Result<SigningKey, ChatKeysError> {
    match bytes.len() {
        32 => {
            let seed: [u8; 32] = bytes
                .try_into()
                .map_err(|_| ChatKeysError::SigningKeyInvalid("seed conversion failed".into()))?;

            SigningKey::from_bytes(&seed)
                .map_err(|e| ChatKeysError::SigningKeyInvalid(format!("invalid seed: {e}")))
        }
        64 => {
            let priv_bytes: [u8; 64] = bytes
                .try_into()
                .map_err(|_| ChatKeysError::SigningKeyInvalid("keypair conversion failed".into()))?;

            SigningKey::from_keypair_bytes(&priv_bytes)
                .map_err(|e| ChatKeysError::SigningKeyInvalid(format!("invalid 64-byte keypair: {e}")))
        }
        other => Err(ChatKeysError::SigningKeyWrongLen { decoded_len: other }),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

fn encode_base64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
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
    pub fn from_strings(
        key_id: u8,
        aead_key_str: &str,
        signing_private_seed_or_bytes_str: &str,
    ) -> Result<Self, ChatKeysError> {
        let key_id_u16 = key_id as u16;
        if key_id_u16 > 255 {
            return Err(ChatKeysError::KeyIdOutOfRange(key_id_u16));
        }

        let aead_decoded = decode_hex_or_b64(aead_key_str)?;
        if aead_decoded.len() != 32 {
            return Err(ChatKeysError::AeadKeyWrongLen(aead_decoded.len()));
        }
        let aead_key: [u8; 32] = aead_decoded
            .try_into()
            .map_err(|_| ChatKeysError::AeadKeyWrongLen(aead_decoded.len()))?;

        let signing_bytes = decode_hex_or_b64(signing_private_seed_or_bytes_str)?;
        let signing_private = decode_signing_key_seed_or_keypair(signing_bytes)?;

        let signing_verify = signing_private.verifying_key();

        Ok(Self {
            key_id,
            aead_key,
            signing_private,
            signing_verify,
        })
    }

    pub fn verify_from_hex(verify_hex: &str) -> Result<VerifyingKey, ChatKeysError> {
        let bytes = hex::decode(verify_hex).map_err(|e| ChatKeysError::DecodeFailed(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(ChatKeysError::SigningKeyWrongLen { decoded_len: bytes.len() });
        }

        let arr: [u8; 32] = bytes.try_into().map_err(|_| {
            ChatKeysError::SigningKeyInvalid("verifying key conversion failed".into())
        })?;

        VerifyingKey::from_bytes(&arr).map_err(|e| {
            ChatKeysError::SigningKeyInvalid(format!("invalid verifying key: {e}"))
        })
    }

    // --- AEAD encoding helpers ---

    pub fn aead_key_hex(&self) -> String {
        encode_hex(&self.aead_key)
    }

    pub fn aead_key_b64(&self) -> String {
        encode_base64(&self.aead_key)
    }

    // --- Ed25519 verify encoding helpers ---

    pub fn signing_verify_hex(&self) -> String {
        encode_hex(&self.signing_verify.to_bytes())
    }

    pub fn signing_verify_b64(&self) -> String {
        encode_base64(&self.signing_verify.to_bytes())
    }

    // --- Ed25519 signing private encoding helpers ---

    /// Output the 32-byte seed representation (ed25519-dalek v2 SigningKey::to_bytes()).
    pub fn signing_private_seed_hex(&self) -> String {
        encode_hex(&self.signing_private.to_bytes())
    }

    pub fn signing_private_seed_b64(&self) -> String {
        encode_base64(&self.signing_private.to_bytes())
    }

    /// Output the 64-byte private-key/keypair bytes representation.
    pub fn signing_private_keypair_64_hex(&self) -> String {
        encode_hex(&self.signing_private.to_keypair_bytes())
    }

    pub fn signing_private_keypair_64_b64(&self) -> String {
        encode_base64(&self.signing_private.to_keypair_bytes())
    }

    // --- Round-trip friendly combined outputs ---

    /// Encode signing private either as:
    /// - 32-byte seed (when `as_seed == true`)
    /// - 64-byte private-key bytes (when `as_seed == false`)
    pub fn signing_private_as_hex(&self, as_seed: bool) -> String {
        if as_seed {
            self.signing_private_seed_hex()
        } else {
            self.signing_private_keypair_64_hex()
        }
    }

    /// Encode signing private either as:
    /// - 32-byte seed (when `as_seed == true`)
    /// - 64-byte private-key bytes (when `as_seed == false`)
    pub fn signing_private_as_b64(&self, as_seed: bool) -> String {
        if as_seed {
            self.signing_private_seed_b64()
        } else {
            self.signing_private_keypair_64_b64()
        }
    }
}
