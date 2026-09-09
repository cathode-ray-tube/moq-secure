use sha2::{Digest, Sha256};

pub const NONCE_PREFIX_5: [u8; 5] = *b"nonce";

pub fn derive_nonce12(key_id: u8, ctr: u64) -> [u8; 12] {
    let mut hasher = Sha256::new();
    hasher.update(NONCE_PREFIX_5);
    hasher.update([key_id]);
    hasher.update(ctr.to_be_bytes());

    let digest = hasher.finalize();

    digest[..12]
        .try_into()
        .expect("SHA-256 slice is exactly 12 bytes")
}

