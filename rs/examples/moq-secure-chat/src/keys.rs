use base64::Engine;
use ed25519_dalek::{SigningKey, VerifyingKey};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatKeysError {
    #[error("aead key must be exactly 32 bytes (decoded {0} bytes)")]
    AeadKeyWrongLen(usize),

    #[error(
        "Ed25519 signing seed must be exactly 32 bytes \
         (decoded {0} bytes); provide the seed, not a private-key file \
         or 64-byte private-key representation"
    )]
    SigningSeedWrongLen(usize),

    #[error(
        "Ed25519 verifying key must be exactly 32 bytes \
         (decoded {0} bytes)"
    )]
    SigningVerifyKeyWrongLen(usize),

    #[error("failed to decode key string as hex or base64: {0}")]
    DecodeFailed(String),

    #[error("invalid Ed25519 signing or verification key: {0}")]
    SigningKeyInvalid(String),
}

fn decode_hex_or_base64(value: &str) -> Result<Vec<u8>, ChatKeysError> {
    let value = value.trim();

    if let Ok(bytes) = hex::decode(value) {
        return Ok(bytes);
    }

    base64::engine::general_purpose::STANDARD
        .decode(value)
        .or_else(|_| {
            base64::engine::general_purpose::STANDARD_NO_PAD.decode(value)
        })
        .map_err(|error| ChatKeysError::DecodeFailed(error.to_string()))
}

fn decode_aead_key(value: &str) -> Result<[u8; 32], ChatKeysError> {
    let bytes = decode_hex_or_base64(value)?;
    let decoded_len = bytes.len();

    bytes
        .try_into()
        .map_err(|_| ChatKeysError::AeadKeyWrongLen(decoded_len))
}

fn decode_ed25519_signing_seed(
    value: &str,
) -> Result<SigningKey, ChatKeysError> {
    let bytes = decode_hex_or_base64(value)?;
    let decoded_len = bytes.len();

    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ChatKeysError::SigningSeedWrongLen(decoded_len))?;

    // ed25519-dalek derives the signing scalar, nonce prefix, and public
    // verification key from this 32-byte Ed25519 seed.
    Ok(SigningKey::from_bytes(&seed))
}

fn decode_ed25519_verifying_key(
    value: &str,
) -> Result<VerifyingKey, ChatKeysError> {
    let bytes = decode_hex_or_base64(value)?;
    let decoded_len = bytes.len();

    let key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ChatKeysError::SigningVerifyKeyWrongLen(decoded_len))?;

    VerifyingKey::from_bytes(&key_bytes).map_err(|error| {
        ChatKeysError::SigningKeyInvalid(error.to_string())
    })
}

/// Key material required by a publisher.
///
/// The signing input is exactly a 32-byte Ed25519 seed, encoded as hex or
/// Base64. It is not a PEM, PKCS#8, OpenSSH, expanded, or 64-byte private
/// key representation.
#[derive(Debug, Clone)]
pub struct PublisherKeys {
    pub key_id: u8,
    pub aead_key: [u8; 32],
    pub signing_key: SigningKey,
}

impl PublisherKeys {
    /// Constructs publisher keys from:
    ///
    /// - an AEAD key encoded as hex or Base64 and decoding to 32 bytes;
    /// - an Ed25519 signing seed encoded as hex or Base64 and decoding to
    ///   exactly 32 bytes.
    pub fn from_strings(
        key_id: u8,
        aead_key: &str,
        ed25519_signing_seed: &str,
    ) -> Result<Self, ChatKeysError> {
        Ok(Self {
            key_id,
            aead_key: decode_aead_key(aead_key)?,
            signing_key: decode_ed25519_signing_seed(
                ed25519_signing_seed,
            )?,
        })
    }

    pub fn signing_verify_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn signing_verify_hex(&self) -> String {
        hex::encode(self.signing_verify_key().to_bytes())
    }

    pub fn signing_verify_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD
            .encode(self.signing_verify_key().to_bytes())
    }

    /// Returns the original 32-byte seed representation.
    ///
    /// `ed25519-dalek` stores the seed inside `SigningKey`; this is not an
    /// expanded 64-byte private-key representation.
    pub fn signing_seed_hex(&self) -> String {
        hex::encode(self.signing_key.to_bytes())
    }

    pub fn signing_seed_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD
            .encode(self.signing_key.to_bytes())
    }

    pub fn aead_key_hex(&self) -> String {
        hex::encode(self.aead_key)
    }

    pub fn aead_key_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.aead_key)
    }
}

/// Key material required by a subscriber.
///
/// A subscriber has no signing private key. It only verifies signatures
/// using the publisher's 32-byte Ed25519 public verification key.
#[derive(Debug, Clone)]
pub struct SubscriberKeys {
    pub key_id: u8,
    pub aead_key: [u8; 32],
    pub signing_verify_key: VerifyingKey,
}

impl SubscriberKeys {
    /// Constructs subscriber keys from:
    ///
    /// - an AEAD key encoded as hex or Base64 and decoding to 32 bytes;
    /// - an Ed25519 public verification key encoded as hex or Base64 and
    ///   decoding to exactly 32 bytes.
    pub fn from_strings(
        key_id: u8,
        aead_key: &str,
        signing_public_key: &str,
    ) -> Result<Self, ChatKeysError> {
        Ok(Self {
            key_id,
            aead_key: decode_aead_key(aead_key)?,
            signing_verify_key: decode_ed25519_verifying_key(
                signing_public_key,
            )?,
        })
    }

    pub fn signing_verify_hex(&self) -> String {
        hex::encode(self.signing_verify_key.to_bytes())
    }

    pub fn signing_verify_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD
            .encode(self.signing_verify_key.to_bytes())
    }

    pub fn aead_key_hex(&self) -> String {
        hex::encode(self.aead_key)
    }

    pub fn aead_key_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.aead_key)
    }
}
