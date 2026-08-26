mod common;

use common::{hex_decode, read_vectors};

use ed25519_dalek::{SigningKey, VerifyingKey};
use moq_secure::{
    decrypt_frame,
    encrypt_frame,
    InMemoryKeyStore,
};

#[test]
fn rust_nonce_vectors_match_js() {
    let vectors = read_vectors();

    for vector in vectors.nonce_vectors {
        let ctr: u64 = vector.ctr.parse().expect("valid counter");
        let actual = moq_secure::derive_nonce12(vector.key_id, ctr);

        assert_eq!(
            hex::encode(actual),
            vector.nonce,
            "nonce mismatch for key_id={} ctr={}",
            vector.key_id,
            ctr,
        );
    }
}

#[test]
fn rust_consumes_js_frames() {
    let vectors = read_vectors();

    let key_bytes = hex_decode(&vectors.aead_key);
    let key: [u8; 32] = key_bytes.try_into().expect("32-byte key");

    let seed_bytes = hex_decode(&vectors.ed25519_seed);
    let seed: [u8; 32] = seed_bytes.try_into().expect("32-byte seed");

    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = VerifyingKey::from(&signing_key);

    let mut keys = [[0u8; 32]; 256];
    keys[7] = key;

    for vector in vectors.frames {
        let frame_bytes = hex_decode(&vector.frame);
        let expected_plaintext = hex_decode(&vector.plaintext);

        let mut lease_remaining = 0u8;

        let plaintext = decrypt_frame(
            &keys,
            &verifying_key,
            &mut lease_remaining,
            &frame_bytes,
        )
        .unwrap_or_else(|error| {
            panic!("Rust failed to consume {}: {error}", vector.name)
        });

        assert_eq!(
            plaintext,
            expected_plaintext,
            "plaintext mismatch for {}",
            vector.name,
        );
    }
}

#[test]
fn rust_produces_the_same_frames_as_js() {
    let vectors = read_vectors();

    let key_bytes = hex_decode(&vectors.aead_key);
    let key: [u8; 32] = key_bytes.try_into().expect("32-byte key");

    let seed_bytes = hex_decode(&vectors.ed25519_seed);
    let seed: [u8; 32] = seed_bytes.try_into().expect("32-byte seed");

    let signing_key = SigningKey::from_bytes(&seed);

    let mut keys = [[0u8; 32]; 256];
    keys[7] = key;

    for vector in vectors.frames {
        let frame_bytes = hex_decode(&vector.frame);

        let parsed = moq_secure::Frame::parse(&frame_bytes)
            .expect("JS frame should parse in Rust");

        let generated = encrypt_frame(
            &keys,
            &signing_key,
            parsed.header.key_id,
            parsed.header.ctr,
            parsed.header.n_signed,
            parsed.header.sig_flag == 1,
            parsed.header.encrypted,
            parsed.header.pad_len,
            &hex_decode(&vector.plaintext),
        )
        .expect("Rust encryption should succeed");

        assert_eq!(
            generated.serialize(),
            frame_bytes,
            "wire mismatch for {}",
            vector.name,
        );
    }
}
