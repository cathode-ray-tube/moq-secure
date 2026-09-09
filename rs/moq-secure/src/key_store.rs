use base64::Engine;
use zeroize::{Zeroize, Zeroizing};

pub trait KeyStore {
    fn aead_key(&self, key_id: u8) -> Option<&[u8; 32]>;
}

#[derive(Debug, thiserror::Error)]
pub enum KeyStoreError {
    #[error("key_id slot {0} is not loaded")]
    KeyNotLoaded(u8),

    #[error("expected 32 bytes (decoded {0} bytes)")]
    KeyWrongLength(usize),

    #[error("failed to decode key as hex/base64")]
    DecodeFailed(#[from] DecodeFailed),
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeFailed {
    #[error("hex decode failed")]
    Hex(#[from] hex::FromHexError),

    #[error("base64 decode failed")]
    Base64(#[from] base64::DecodeError),

    #[error("key string looked like hex but had the wrong length")]
    HexWrongLength,

    #[error("unknown decode error")]
    Other,
}

#[derive(Debug)]
pub struct InMemoryKeyStore {
    // Unloaded slots contain no key material.
    // Loaded keys are zeroized when removed or when the store is dropped.
    keys: [Option<Zeroizing<[u8; 32]>>; 256],
}

impl InMemoryKeyStore {
    pub fn empty() -> Self {
        Self {
            // `from_fn` avoids initializing the backing storage with
            // hard-coded cryptographic-looking byte arrays.
            keys: std::array::from_fn(|_| None),
        }
    }

    pub fn set_key(&mut self, key_id: u8, key: [u8; 32]) {
        self.keys[key_id as usize] = Some(Zeroizing::new(key));
    }

    /// Accepts either:
    /// - hex: 64 hex characters representing 32 bytes
    /// - base64: decodes to exactly 32 bytes, with or without padding
    pub fn set_key_encoded(
        &mut self,
        key_id: u8,
        key_encoded: &str,
    ) -> Result<(), KeyStoreError> {
        let key_encoded = key_encoded.trim();

        // Try hex only when the input has exactly the expected hex length.
        // This avoids accidentally treating arbitrary base64 as hex.
        if key_encoded.len() == 64
            && key_encoded
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            let decoded = hex::decode(key_encoded).map_err(DecodeFailed::from)?;
            let decoded = Zeroizing::new(decoded);

            let key: [u8; 32] = decoded
                .as_slice()
                .try_into()
                .map_err(|_| KeyStoreError::KeyWrongLength(decoded.len()))?;

            self.set_key(key_id, key);
            return Ok(());
        }

        // Keep decoded key bytes in zeroizing storage while converting them.
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(key_encoded)
            .or_else(|_| {
                base64::engine::general_purpose::STANDARD_NO_PAD.decode(key_encoded)
            })
            .map_err(DecodeFailed::from)?;

        let decoded = Zeroizing::new(decoded);

        if decoded.len() != 32 {
            return Err(KeyStoreError::KeyWrongLength(decoded.len()));
        }

        let key: [u8; 32] = decoded
            .as_slice()
            .try_into()
            .map_err(|_| KeyStoreError::KeyWrongLength(decoded.len()))?;

        self.set_key(key_id, key);
        Ok(())
    }

    pub fn remove_key(&mut self, key_id: u8) -> Result<(), KeyStoreError> {
        self.keys[key_id as usize]
            .take()
            .map(|_| ())
            .ok_or(KeyStoreError::KeyNotLoaded(key_id))
    }

    pub fn contains_key(&self, key_id: u8) -> bool {
        self.keys[key_id as usize].is_some()
    }
}

impl KeyStore for InMemoryKeyStore {
    fn aead_key(&self, key_id: u8) -> Option<&[u8; 32]> {
        self.keys[key_id as usize]
            .as_ref()
            .map(|key| &**key)
    }
}

