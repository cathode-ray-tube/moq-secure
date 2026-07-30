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

    #[error("encrypted flag must be 0 or 1, got {0}")]
    InvalidEncryptedFlag(u8),

    #[error("sigFlag must be 0 or 1, got {0}")]
    InvalidSigFlag(u8),

    #[error("AEAD authentication failed")]
    AeadAuthFailed,

    #[error("signature invalid or signature verification failed")]
    InvalidSignature,

    #[error("signing is disabled but sigFlag indicates signature or sigSlot non-zero")]
    SigningMismatch,

    #[error("signing enabled but sigFlag indicates signature while sigSlot is missing/zero")]
    MissingSigSlot,

    #[error("signature present (sigFlag=1) but nSigned is 0")]
    SignatureNotAllowedByNSigned,

    #[error("decryption failed")]
    DecryptFailed,
}
