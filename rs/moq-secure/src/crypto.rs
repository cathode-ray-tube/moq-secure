use std::convert::TryFrom;

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm,
};
use chacha20poly1305::{
    aead::Payload as ChaChaPayload,
    ChaCha20Poly1305,
};
use sha2::{Digest, Sha256};

use crate::nonce::derive_nonce12;
use crate::MoqSecureError;

pub const AEAD_TAG_LEN: usize = 16;

#[allow(dead_code)]
pub(crate) const ENCRYPTION_UNENCRYPTED: u8 = 0;
pub(crate) const ENCRYPTION_CHACHA20_POLY1305: u8 = 1;
pub(crate) const ENCRYPTION_AES_256_GCM: u8 = 2;

pub(crate) fn sha256_digest(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);

    hasher.finalize().into()
}

fn split_ciphertext_and_tag(
    combined: &[u8],
) -> (Vec<u8>, [u8; AEAD_TAG_LEN]) {
    debug_assert!(combined.len() >= AEAD_TAG_LEN);

    let ciphertext_len = combined.len() - AEAD_TAG_LEN;
    let ciphertext = combined[..ciphertext_len].to_vec();

    let mut tag = [0u8; AEAD_TAG_LEN];
    tag.copy_from_slice(&combined[ciphertext_len..]);

    (ciphertext, tag)
}

fn combine_ciphertext_and_tag(
    ciphertext: &[u8],
    tag: &[u8; AEAD_TAG_LEN],
) -> Vec<u8> {
    let mut combined =
        Vec::with_capacity(ciphertext.len() + AEAD_TAG_LEN);

    combined.extend_from_slice(ciphertext);
    combined.extend_from_slice(tag);

    combined
}

fn encrypt_chacha20_poly1305(
    key: &[u8; 32],
    key_id: u8,
    ctr: u64,
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; AEAD_TAG_LEN]) {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .expect("ChaCha20-Poly1305 key must be 32 bytes");

    let nonce_bytes = derive_nonce12(key_id, ctr);
    let nonce = chacha20poly1305::Nonce::try_from(nonce_bytes.as_slice())
        .expect("ChaCha20-Poly1305 nonce must be 12 bytes");

    let combined = cipher
        .encrypt(
            &nonce,
            ChaChaPayload {
                msg: plaintext,
                aad,
            },
        )
        .expect("ChaCha20-Poly1305 encryption failure should be impossible");

    split_ciphertext_and_tag(&combined)
}

fn encrypt_aes256_gcm(
    key: &[u8; 32],
    key_id: u8,
    ctr: u64,
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; AEAD_TAG_LEN]) {
    let cipher = Aes256Gcm::new_from_slice(key)
        .expect("AES-256-GCM key must be 32 bytes");

    let nonce_bytes = derive_nonce12(key_id, ctr);
    let nonce = aes_gcm::Nonce::try_from(nonce_bytes.as_slice())
        .expect("AES-256-GCM nonce must be 12 bytes");

    let combined = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("AES-256-GCM encryption failure should be impossible");

    split_ciphertext_and_tag(&combined)
}

/// Encrypt according to the wire-level encryption type:
///
/// 1 = ChaCha20-Poly1305
/// 2 = AES-256-GCM
pub(crate) fn aead_encrypt(
    encryption_type: u8,
    key: &[u8; 32],
    key_id: u8,
    ctr: u64,
    aad: &[u8],
    plaintext: &[u8],
) -> Result<(Vec<u8>, [u8; AEAD_TAG_LEN]), MoqSecureError> {
    match encryption_type {
        ENCRYPTION_CHACHA20_POLY1305 => Ok(encrypt_chacha20_poly1305(
            key,
            key_id,
            ctr,
            aad,
            plaintext,
        )),

        ENCRYPTION_AES_256_GCM => Ok(encrypt_aes256_gcm(
            key,
            key_id,
            ctr,
            aad,
            plaintext,
        )),

        other => Err(MoqSecureError::UnsupportedAlgorithm(other)),
    }
}

fn decrypt_chacha20_poly1305(
    key: &[u8; 32],
    key_id: u8,
    ctr: u64,
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; AEAD_TAG_LEN],
) -> Result<Vec<u8>, MoqSecureError> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .expect("ChaCha20-Poly1305 key must be 32 bytes");

    let nonce_bytes = derive_nonce12(key_id, ctr);
    let nonce = chacha20poly1305::Nonce::try_from(nonce_bytes.as_slice())
        .expect("ChaCha20-Poly1305 nonce must be 12 bytes");

    let combined = combine_ciphertext_and_tag(ciphertext, tag);

    cipher
        .decrypt(
            &nonce,
            ChaChaPayload {
                msg: &combined,
                aad,
            },
        )
        .map_err(|_| MoqSecureError::AeadAuthFailed)
}

fn decrypt_aes256_gcm(
    key: &[u8; 32],
    key_id: u8,
    ctr: u64,
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; AEAD_TAG_LEN],
) -> Result<Vec<u8>, MoqSecureError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .expect("AES-256-GCM key must be 32 bytes");

    let nonce_bytes = derive_nonce12(key_id, ctr);
    let nonce = aes_gcm::Nonce::try_from(nonce_bytes.as_slice())
        .expect("AES-256-GCM nonce must be 12 bytes");

    let combined = combine_ciphertext_and_tag(ciphertext, tag);

    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: &combined,
                aad,
            },
        )
        .map_err(|_| MoqSecureError::AeadAuthFailed)
}

/// Decrypt according to the wire-level encryption type:
///
/// 1 = ChaCha20-Poly1305
/// 2 = AES-256-GCM
pub(crate) fn aead_decrypt(
    encryption_type: u8,
    key: &[u8; 32],
    key_id: u8,
    ctr: u64,
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; AEAD_TAG_LEN],
) -> Result<Vec<u8>, MoqSecureError> {
    match encryption_type {
        ENCRYPTION_CHACHA20_POLY1305 => decrypt_chacha20_poly1305(
            key,
            key_id,
            ctr,
            aad,
            ciphertext,
            tag,
        ),

        ENCRYPTION_AES_256_GCM => decrypt_aes256_gcm(
            key,
            key_id,
            ctr,
            aad,
            ciphertext,
            tag,
        ),

        other => Err(MoqSecureError::UnsupportedAlgorithm(other)),
    }
}
