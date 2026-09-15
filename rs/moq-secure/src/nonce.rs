pub const NONCE_PREFIX_3: [u8; 3] = *b"non";

pub fn derive_nonce12(key_id: u8, ctr: u64) -> [u8; 12] {
    let ctr = ctr.to_be_bytes();

    [
        NONCE_PREFIX_3[0],
        NONCE_PREFIX_3[1],
        NONCE_PREFIX_3[2],
        key_id,
        ctr[0],
        ctr[1],
        ctr[2],
        ctr[3],
        ctr[4],
        ctr[5],
        ctr[6],
        ctr[7],
    ]
}
