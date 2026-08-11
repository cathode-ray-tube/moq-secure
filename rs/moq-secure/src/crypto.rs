// crypto.rs

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

fn split_ciphertext_and_tag(combined: &[u8]) -> (Vec<u8>, [u8; AEAD_TAG_LEN]) {
    debug_assert!(combined.len() >= AEAD_TAG_LEN);
    let ct_len = combined.len() - AEAD_TAG_LEN;

    let ciphertext = combined[..ct_len].to_vec();

    let mut tag = [0u8; AEAD_TAG_LEN];
    tag.copy_from_slice(&combined[ct_len..]);
    (ciphertext, tag)
}

fn combine_ciphertext_and_tag(ciphertext: &[u8], tag: &[u8; AEAD_TAG_LEN]) -> Vec<u8> {
    let mut combined = Vec::with_capacity(ciphertext.len() + AEAD_TAG_LEN);
    combined.extend_from_slice(ciphertext);
    combined.extend_from_slice(tag);
    combined
}

/// Encrypt with ChaCha20-Poly1305.
///
/// This returns:
/// - ciphertext: the encrypted bytes (length == plaintext length)
/// - tag: 16-byte Poly1305 authentication tag
///
/// Note: `aad` must exclude any signature bytes; it should be limited to what the
/// frame spec requires (e.g., unencrypted header / associated data), and never the
/// Ed25519 signature trailer.
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

    let out = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("encryption failure should be impossible");

    split_ciphertext_and_tag(&out)
}

/// Decrypt with ChaCha20-Poly1305.
///
/// `ciphertext` must not include the tag; the `tag` is provided separately as 16 bytes.
///
/// Note: `aad` must exclude any signature bytes; it must match exactly the AAD
/// used during encryption.
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

    let combined = combine_ciphertext_and_tag(ciphertext, tag);

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
