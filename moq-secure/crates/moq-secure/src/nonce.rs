use sha2::{Digest, Sha256};

pub const NONCE_PREFIX_5: [u8; 5] = *b"nonce";

pub fn derive_nonce12(key_id: u8, ctr: u64) -> [u8; 12] {
    let mut hasher = Sha256::new();
    hasher.update(&NONCE_PREFIX_5);
    hasher.update([key_id]);
    hasher.update(ctr.to_be_bytes());
    let digest = hasher.finalize();
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&digest[..12]);
    nonce
}
