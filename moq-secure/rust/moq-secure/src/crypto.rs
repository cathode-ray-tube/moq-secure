use chacha20poly1305::{
    aead::{Aead, Nonce},
    ChaCha20Poly1305, Key,
};
use sha2::Sha256;

use crate::nonce::derive_nonce12;

pub fn aead_encrypt(
    key: &[u8; 32],
    key_id: u8,
    ctr: u64,
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; 16]) {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce12 = derive_nonce12(key_id, ctr);
    let nonce = Nonce::from_slice(&nonce12);

    let tag_and_ct = cipher
        .encrypt(
            nonce,
            chacha20poly1305::aead::Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("encryption should not fail");

    let ct_len = plaintext.len();
    let ciphertext = &tag_and_ct[..ct_len];
    let tag = &tag_and_ct[ct_len..ct_len + 16];

    (ciphertext.to_vec(), tag.try_into().expect("16 bytes"))
}

pub fn aead_decrypt(
    key: &[u8; 32],
    key_id: u8,
    ctr: u64,
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; 16],
) -> Result<Vec<u8>, ()> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce12 = derive_nonce12(key_id, ctr);
    let nonce = Nonce::from_slice(&nonce12);

    let mut ct_and_tag = Vec::with_capacity(ciphertext.len() + 16);
    ct_and_tag.extend_from_slice(ciphertext);
    ct_and_tag.extend_from_slice(tag);

    cipher
        .decrypt(
            nonce,
            chacha20poly1305::aead::Payload {
                msg: &ct_and_tag,
                aad,
            },
        )
        .map_err(|_| ())
}

pub fn sha256_digest(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let out = hasher.finalize();
    let mut d = [0u8; 32];
    d.copy_from_slice(&out);
    d
}
