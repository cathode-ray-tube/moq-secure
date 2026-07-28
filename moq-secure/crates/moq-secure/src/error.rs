use thiserror::Error;

#[derive(Debug, Error)]
pub enum MoqSecureError {
    #[error("invalid magic bytes")]
    InvalidMagic,

    #[error("unsupported version: {0}")]
    UnsupportedVersion(u8),

    #[error("not enough bytes in frame")]
    TruncatedFrame,

    #[error("ciphertext too short for AEAD tag")]
    CiphertextTooShort,

    #[error("AEAD authentication failed")]
    AeadAuthFailed,

    #[error("signature requested but signature is invalid")]
    InvalidSignature,

    #[error("signing is disabled but sigFlag indicates signature or sigSlot non-zero")]
    SigningMismatch,

    #[error("decrypt failed")]
    DecryptFailed,
}
