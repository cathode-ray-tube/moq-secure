use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct VectorFile {
    pub version: u8,
    #[serde(rename = "aeadKey")]
    pub aead_key: String,
    #[serde(rename = "ed25519Seed")]
    pub ed25519_seed: String,
    #[serde(rename = "nonceVectors")]
    pub nonce_vectors: Vec<NonceVector>,
    pub frames: Vec<FrameVector>,
}

#[derive(Debug, Deserialize)]
pub struct NonceVector {
    #[serde(rename = "keyId")]
    pub key_id: u8,
    pub ctr: String,
    pub nonce: String,
}

#[derive(Debug, Deserialize)]
pub struct FrameVector {
    pub name: String,
    pub plaintext: String,

    #[serde(rename = "padLen")]
    pub pad_len: u32,

    pub frame: String,
    pub header: String,
    pub payload: String,
    pub tag: String,
    pub signature: Option<String>,
    pub lease: u8,
}

pub fn read_vectors() -> VectorFile {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test-vectors/frames.json"
    )))
    .expect("failed to parse test-vectors/frames.json")
}

pub fn hex_decode(value: &str) -> Vec<u8> {
    hex::decode(value).expect("valid hex")
}
