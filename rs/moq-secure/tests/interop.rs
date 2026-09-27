mod common;

use common::{hex_decode, read_vectors};

use ed25519_dalek::{SigningKey, VerifyingKey};
use moq_secure::{
    decrypt_frame,
    encrypt_frame,
    Frame,
    InMemoryKeyStore,
    MoqSecureError,
};

const ENCRYPTION_UNENCRYPTED: u8 = 0;
const ENCRYPTION_CHACHA20_POLY1305: u8 = 1;
const ENCRYPTION_AES_256_GCM: u8 = 2;

fn test_keys() -> (InMemoryKeyStore, SigningKey, VerifyingKey) {
    let vectors = read_vectors();

    let key_bytes = hex_decode(&vectors.aead_key);
    let key: [u8; 32] = key_bytes.try_into().expect("32-byte AEAD key");

    let seed_bytes = hex_decode(&vectors.ed25519_seed);
    let seed: [u8; 32] = seed_bytes.try_into().expect("32-byte Ed25519 seed");

    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = VerifyingKey::from(&signing_key);

    let mut key_store = InMemoryKeyStore::empty();
    key_store.set_key(7, key);

    (key_store, signing_key, verifying_key)
}

#[test]
fn vector_file_has_expected_version() {
    let vectors = read_vectors();

    assert_eq!(vectors.version, 1);
    assert_eq!(vectors.aead_key.len(), 64);
    assert_eq!(vectors.ed25519_seed.len(), 64);
    assert!(!vectors.nonce_vectors.is_empty());
    assert!(!vectors.frames.is_empty());
}

#[test]
fn vector_file_contains_all_encryption_types() {
    let vectors = read_vectors();

    let mut has_unencrypted = false;
    let mut has_chacha = false;
    let mut has_aes = false;

    for vector in vectors.frames {
        let frame = Frame::parse(&hex_decode(&vector.frame))
            .expect("generated vector should parse");

        match frame.header.encryption_type {
            ENCRYPTION_UNENCRYPTED => has_unencrypted = true,
            ENCRYPTION_CHACHA20_POLY1305 => has_chacha = true,
            ENCRYPTION_AES_256_GCM => has_aes = true,
            value => panic!("unexpected encryption type {value}"),
        }
    }

    assert!(has_unencrypted, "missing unencrypted vector");
    assert!(has_chacha, "missing ChaCha20-Poly1305 vector");
    assert!(has_aes, "missing AES-256-GCM vector");
}

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
fn rust_parses_every_js_frame() {
    let vectors = read_vectors();

    for vector in vectors.frames {
        let frame_bytes = hex_decode(&vector.frame);
        let frame = Frame::parse(&frame_bytes)
            .unwrap_or_else(|error| {
                panic!("failed to parse {}: {error}", vector.name)
            });

        assert_eq!(
            frame.serialize(),
            frame_bytes,
            "parse/serialize mismatch for {}",
            vector.name,
        );

        assert_eq!(
            frame.header.encode(),
            hex_decode(&vector.header),
            "header mismatch for {}",
            vector.name,
        );

        assert_eq!(
            frame.payload,
            hex_decode(&vector.payload),
            "payload mismatch for {}",
            vector.name,
        );

        assert_eq!(
            hex::encode(frame.tag),
            vector.tag,
            "tag mismatch for {}",
            vector.name,
        );

        match (&frame.signature, &vector.signature) {
            (None, None) => {}
            (Some(actual), Some(expected)) => {
                assert_eq!(
                    hex::encode(actual),
                    *expected,
                    "signature mismatch for {}",
                    vector.name,
                );
            }
            (None, Some(_)) => {
                panic!("missing signature for {}", vector.name)
            }
            (Some(_), None) => {
                panic!("unexpected signature for {}", vector.name)
            }
        }
    }
}

#[test]
fn rust_consumes_js_frames() {
    let vectors = read_vectors();
    let (key_store, _signing_key, verifying_key) = test_keys();

    let mut lease_remaining = 0u8;

    for vector in vectors.frames {
        assert_eq!(
            lease_remaining,
            vector.initial_lease,
            "initial lease mismatch for {}",
            vector.name,
        );

        let frame_bytes = hex_decode(&vector.frame);
        let expected_plaintext = hex_decode(&vector.plaintext);

        let plaintext = decrypt_frame(
            &key_store,
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

        assert_eq!(
            lease_remaining,
            vector.lease,
            "lease mismatch after {}",
            vector.name,
        );
    }
}

#[test]
fn rust_produces_the_same_frames_as_js() {
    let vectors = read_vectors();
    let (key_store, signing_key, _verifying_key) = test_keys();

    for vector in vectors.frames {
        let frame_bytes = hex_decode(&vector.frame);

        let parsed = Frame::parse(&frame_bytes)
            .unwrap_or_else(|error| {
                panic!("JS frame should parse in Rust: {error}")
            });

        let generated = encrypt_frame(
            &key_store,
            &signing_key,
            parsed.header.key_id,
            parsed.header.ctr,
            parsed.header.n_signed,
            parsed.header.sig_flag == 1,
            parsed.header.encryption_type,
            vector.pad_len,
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

#[test]
fn every_vector_round_trips_through_rust() {
    let vectors = read_vectors();
    let (key_store, signing_key, verifying_key) = test_keys();

    for vector in vectors.frames {
        let expected_plaintext = hex_decode(&vector.plaintext);

        let original = Frame::parse(&hex_decode(&vector.frame))
            .expect("vector frame should parse");

        let generated = encrypt_frame(
            &key_store,
            &signing_key,
            original.header.key_id,
            original.header.ctr,
            original.header.n_signed,
            original.header.sig_flag == 1,
            original.header.encryption_type,
            vector.pad_len,
            &expected_plaintext,
        )
        .expect("Rust encryption should succeed");

        let mut lease_remaining = vector.initial_lease;

        let decrypted = decrypt_frame(
            &key_store,
            &verifying_key,
            &mut lease_remaining,
            &generated.serialize(),
        )
        .unwrap_or_else(|error| {
            panic!("round-trip decryption failed for {}: {error}", vector.name)
        });

        assert_eq!(
            decrypted,
            expected_plaintext,
            "round-trip plaintext mismatch for {}",
            vector.name,
        );

        assert_eq!(
            lease_remaining,
            vector.lease,
            "round-trip lease mismatch for {}",
            vector.name,
        );
    }
}

#[test]
fn aes256gcm_vectors_are_individually_usable() {
    let vectors = read_vectors();
    let (key_store, signing_key, verifying_key) = test_keys();

    for vector in vectors.frames {
        let frame = Frame::parse(&hex_decode(&vector.frame))
            .expect("vector frame should parse");

        if frame.header.encryption_type != ENCRYPTION_AES_256_GCM {
            continue;
        }

        let plaintext = hex_decode(&vector.plaintext);
        let mut lease_remaining = vector.initial_lease;

        let decrypted = decrypt_frame(
            &key_store,
            &verifying_key,
            &mut lease_remaining,
            &hex_decode(&vector.frame),
        )
        .expect("AES-256-GCM vector should decrypt");

        assert_eq!(decrypted, plaintext);

        let regenerated = encrypt_frame(
            &key_store,
            &signing_key,
            frame.header.key_id,
            frame.header.ctr,
            frame.header.n_signed,
            frame.header.sig_flag == 1,
            frame.header.encryption_type,
            vector.pad_len,
            &plaintext,
        )
        .expect("AES-256-GCM vector should encrypt");

        assert_eq!(
            regenerated.serialize(),
            hex_decode(&vector.frame),
            "AES-256-GCM wire mismatch for {}",
            vector.name,
        );
    }
}

#[test]
fn encryption_type_values_match_wire_format() {
    let vectors = read_vectors();

    for vector in vectors.frames {
        let frame = Frame::parse(&hex_decode(&vector.frame))
            .expect("vector frame should parse");

        match frame.header.encryption_type {
            ENCRYPTION_UNENCRYPTED
            | ENCRYPTION_CHACHA20_POLY1305
            | ENCRYPTION_AES_256_GCM => {}
            value => panic!("unsupported test-vector encryption type {value}"),
        }
    }
}

#[test]
fn vector_headers_have_fixed_wire_length() {
    let vectors = read_vectors();

    for vector in vectors.frames {
        assert_eq!(
            hex_decode(&vector.header).len(),
            17,
            "invalid header length for {}",
            vector.name,
        );
    }
}

#[test]
fn vector_tags_have_expected_length_for_encrypted_frames() {
    let vectors = read_vectors();

    for vector in vectors.frames {
        let frame = Frame::parse(&hex_decode(&vector.frame))
            .expect("vector frame should parse");

        if frame.header.encryption_type == ENCRYPTION_UNENCRYPTED {
            assert_eq!(
                vector.tag,
                "00000000000000000000000000000000",
                "unencrypted tag should be zero-filled for {}",
                vector.name,
            );
        } else {
            assert_eq!(
                hex_decode(&vector.tag).len(),
                16,
                "invalid AEAD tag length for {}",
                vector.name,
            );
        }
    }
}
