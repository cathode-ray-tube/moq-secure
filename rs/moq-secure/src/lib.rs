pub mod crypto;
pub mod error;
pub mod key_store;
pub mod nonce;
pub mod wire;

pub use error::MoqSecureError;

// Re-export key-store types so applications can construct and populate one.
pub use key_store::{InMemoryKeyStore, KeyStore, KeyStoreError};

pub use nonce::derive_nonce12;

pub use wire::{
    decrypt_frame,
    encrypt_frame,
    EncryptionType,
    Frame,
    WireHeader,
    ENCRYPTION_AES_256_GCM,
    ENCRYPTION_CHACHA20_POLY1305,
    ENCRYPTION_UNENCRYPTED,
    FIXED_HEADER_LEN,
    MAGIC,
    VERSION,
};

