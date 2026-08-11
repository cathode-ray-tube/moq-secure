// rust/moq-secure/src/key_store.rs

pub trait KeyStore {
    fn aead_key(&self, key_id: u8) -> Option<&[u8; 32]>;
}

#[derive(Debug, thiserror::Error)]
pub enum KeyStoreError {
    #[error("key_id {0} is required (slot is 0..255)")]
    KeyIdInvalid(u8),

    #[error("expected 32 bytes (decoded {0} bytes)")]
    KeyWrongLength(usize),

    #[error("failed to decode key as hex/base64")]
    DecodeFailed(#[from] DecodeFailed),

    #[error("key_id slot {0} not loaded")]
    KeyNotLoaded(u8),
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeFailed {
    #[error("hex decode failed")]
    Hex(#[from] hex::FromHexError),

    #[error("base64 decode failed")]
    Base64(#[from] base64::DecodeError),

    #[error("key string looked like hex but wrong length")]
    HexWrongLength,

    #[error("unknown decode error")]
    Other,
}

#[derive(Debug, Clone)]
pub struct InMemoryKeyStore {
    keys: [[u8; 32]; 256],
    loaded: [bool; 256],
}

impl InMemoryKeyStore {
    pub fn empty() -> Self {
        Self {
            keys: [[0u8; 32]; 256],
            loaded: [false; 256],
        }
    }

    pub fn set_key(&mut self, key_id: u8, key: [u8; 32]) {
        self.keys[key_id as usize] = key;
        self.loaded[key_id as usize] = true;
    }

    /// Accepts either:
    /// - hex: 64 hex chars (32 bytes)
    /// - base64: encodes to exactly 32 bytes (with or without padding)
    pub fn set_key_encoded(
        &mut self,
        key_id: u8,
        key_encoded: &str,
    ) -> Result<(), KeyStoreError> {
        let key_encoded = key_encoded.trim();

        // Try hex only if it has the exact expected length.
        // (This avoids accidentally treating arbitrary base64 as hex.)
        if key_encoded.len() == 64 && key_encoded.chars().all(|c| c.is_ascii_hexdigit()) {
            let bytes = hex::decode(key_encoded)?; // will be 32 bytes
            let key = <[u8; 32]>::try_from(bytes.as_slice())
                .map_err(|_| KeyStoreError::KeyWrongLength(bytes.len()))?;
            self.set_key(key_id, key);
            return Ok(());
        }

        // Otherwise, try base64.
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(key_encoded)
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(key_encoded))?;

        if decoded.len() != 32 {
            return Err(KeyStoreError::KeyWrongLength(decoded.len()));
        }

        let key: [u8; 32] = decoded
            .try_into()
            .map_err(|_| KeyStoreError::KeyWrongLength(decoded.len()))?;

        self.set_key(key_id, key);
        Ok(())
    }
}

impl KeyStore for InMemoryKeyStore {
    fn aead_key(&self, key_id: u8) -> Option<&[u8; 32]> {
        if self.loaded[key_id as usize] {
            Some(&self.keys[key_id as usize])
        } else {
            None
        }
    }
}
