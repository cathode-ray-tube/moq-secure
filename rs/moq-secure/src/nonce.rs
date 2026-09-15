pub const NONCE_PREFIX_3: [u8; 3] = *b"non";

pub fn derive_nonce12(key_id: u8, ctr: u64) -> [u8; 12] {
	let mut nonce = [0u8; 12];

	nonce[..3].copy_from_slice(NONCE_PREFIX_3);
	nonce[3] = key_id;
	nonce[4..].copy_from_slice(&ctr.to_be_bytes());

	nonce
}
