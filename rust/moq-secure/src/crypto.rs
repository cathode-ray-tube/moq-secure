// rust/moq-secure/src/crypto.rs

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key};
use sha2::{Digest, Sha256};

use crate::MoqSecureError;
use crate::nonce::derive_nonce12;

pub const AEAD_TAG_LEN: usize = 16;

pub(crate) fn sha256_digest(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    digest
        .as_slice()
        .try_into()
        .expect("sha256 output is always 32 bytes")
}

pub(crate) fn aead_encrypt(
    key: &[u8; 32],
    key_id: u8,
    ctr: u64,
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; AEAD_TAG_LEN]) {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce_bytes = derive_nonce12(key_id, ctr);
    let nonce = chacha20poly1305::Nonce::from_slice(&nonce_bytes);

    // chacha20poly1305 returns ciphertext||tag (tag is last 16 bytes)
    let out = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("encryption failure should be impossible");

    let ct_len = out.len() - AEAD_TAG_LEN;
    let ciphertext = out[..ct_len].to_vec();

    let mut tag = [0u8; AEAD_TAG_LEN];
    tag.copy_from_slice(&out[ct_len..]);
    (ciphertext, tag)
}

pub(crate) fn aead_decrypt(
    key: &[u8; 32],
    key_id: u8,
    ctr: u64,
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; AEAD_TAG_LEN],
) -> Result<Vec<u8>, MoqSecureError> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce_bytes = derive_nonce12(key_id, ctr);
    let nonce = chacha20poly1305::Nonce::from_slice(&nonce_bytes);

    let mut combined = Vec::with_capacity(ciphertext.len() + AEAD_TAG_LEN);
    combined.extend_from_slice(ciphertext);
    combined.extend_from_slice(tag);

    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &combined,
                aad,
            },
        )
        .map_err(|_| MoqSecureError::AeadAuthFailed)
}
