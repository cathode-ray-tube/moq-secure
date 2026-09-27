use ed25519_dalek::{SigningKey, VerifyingKey};
use moq_secure::{
    decrypt_frame,
    encrypt_frame,
    Frame,
    InMemoryKeyStore,
    MoqSecureError,
};

const HEADER_LEN: usize = 17;
const TAG_LEN: usize = 16;
const SIGNATURE_LEN: usize = 64;

const ENCRYPTION_UNENCRYPTED: u8 = 0;
const ENCRYPTION_CHACHA20_POLY1305: u8 = 1;
const ENCRYPTION_AES_256_GCM: u8 = 2;

fn valid_header() -> Vec<u8> {
    let mut frame = vec![0u8; HEADER_LEN];

    frame[0..4].copy_from_slice(b"MOQS");
    frame[4] = 1;
    frame[5] = 7;
    frame[14] = 0;
    frame[15] = 0;
    frame[16] = ENCRYPTION_UNENCRYPTED;

    frame
}

fn test_key_store() -> InMemoryKeyStore {
    let mut key_store = InMemoryKeyStore::empty();
    key_store.set_key(7, [0u8; 32]);
    key_store
}

fn test_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[1u8; 32])
}

fn test_verifying_key() -> VerifyingKey {
    VerifyingKey::from(&test_signing_key())
}

fn valid_encrypted_frame(encryption_type: u8) -> Vec<u8> {
    let mut frame = valid_header();
    frame[16] = encryption_type;
    frame.extend_from_slice(&[0u8; TAG_LEN]);
    frame
}

#[test]
fn rejects_short_header() {
    let result = Frame::parse(&vec![0u8; HEADER_LEN - 1]);

    assert!(matches!(
        result,
        Err(MoqSecureError::TruncatedFrame)
    ));
}

#[test]
fn rejects_bad_magic() {
    let mut frame = valid_header();
    frame[0..4].copy_from_slice(b"NOPE");

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::InvalidMagic)
    ));
}

#[test]
fn rejects_invalid_version() {
    let mut frame = valid_header();
    frame[4] = 2;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::UnsupportedVersion(2))
    ));
}

#[test]
fn rejects_invalid_signature_flag() {
    let mut frame = valid_header();
    frame[15] = 2;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::InvalidSigFlag(2))
    ));
}

#[test]
fn rejects_unsupported_encryption_type() {
    let mut frame = valid_header();
    frame[16] = 3;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::UnsupportedAlgorithm(3))
    ));
}

#[test]
fn accepts_chacha20_poly1305_encryption_type() {
    let frame = valid_encrypted_frame(ENCRYPTION_CHACHA20_POLY1305);

    assert!(Frame::parse(&frame).is_ok());
}

#[test]
fn accepts_aes256_gcm_encryption_type() {
    let frame = valid_encrypted_frame(ENCRYPTION_AES_256_GCM);

    assert!(Frame::parse(&frame).is_ok());
}

#[test]
fn rejects_encrypted_body_without_tag() {
    let mut frame = valid_header();
    frame[16] = ENCRYPTION_CHACHA20_POLY1305;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::CiphertextTooShort)
    ));
}

#[test]
fn rejects_aes256_gcm_body_without_tag() {
    let mut frame = valid_header();
    frame[16] = ENCRYPTION_AES_256_GCM;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::CiphertextTooShort)
    ));
}

#[test]
fn rejects_zero_signature() {
    let mut frame = valid_header();

    frame[14] = 1;
    frame[15] = 1;

    frame.extend_from_slice(&[0u8; SIGNATURE_LEN]);

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::InvalidSignature)
    ));
}

#[test]
fn rejects_signature_flag_without_signature_bytes() {
    let mut frame = valid_header();

    frame[14] = 1;
    frame[15] = 1;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::TruncatedFrame)
    ));
}

#[test]
fn rejects_nonzero_signature_flag_when_n_signed_is_zero() {
    let mut frame = valid_header();
    frame[14] = 0;
    frame[15] = 1;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::SigningMismatch)
    ));
}

#[test]
fn rejects_invalid_signing_state_during_decryption() {
    let key_store = test_key_store();
    let verifying_key = test_verifying_key();

    let mut frame = valid_header();
    frame[14] = 0;
    frame[15] = 1;
    frame.extend_from_slice(&[1u8; SIGNATURE_LEN]);

    let mut lease_remaining = 0;

    let result = decrypt_frame(
        &key_store,
        &verifying_key,
        &mut lease_remaining,
        &frame,
    );

    assert!(matches!(
        result,
        Err(MoqSecureError::SigningMismatch)
    ));
}

#[test]
fn rejects_invalid_padding_length_field() {
    let key_store = test_key_store();
    let verifying_key = test_verifying_key();

    let signing_key = test_signing_key();

    let frame = encrypt_frame(
        &key_store,
        &signing_key,
        7,
        1,
        0,
        false,
        ENCRYPTION_UNENCRYPTED,
        0,
        b"payload",
    )
    .expect("frame should encrypt");

    let mut bytes = frame.serialize();

    // The first four bytes after the 17-byte header are pad_len.
    bytes[HEADER_LEN..HEADER_LEN + 4]
        .copy_from_slice(&u32::MAX.to_be_bytes());

    let mut lease_remaining = 0;

    let result = decrypt_frame(
        &key_store,
        &verifying_key,
        &mut lease_remaining,
        &bytes,
    );

    assert!(matches!(
        result,
        Err(MoqSecureError::InvalidPadLength)
    ));
}

#[test]
fn rejects_truncated_plaintext_padding_field() {
    let key_store = test_key_store();
    let verifying_key = test_verifying_key();

    let mut frame = valid_header();

    // An unencrypted payload shorter than the four-byte pad length field.
    frame.extend_from_slice(&[0u8; 3]);

    let mut lease_remaining = 0;

    let result = decrypt_frame(
        &key_store,
        &verifying_key,
        &mut lease_remaining,
        &frame,
    );

    assert!(matches!(
        result,
        Err(MoqSecureError::InvalidPadLength)
    ));
}

#[test]
fn rejects_unknown_key_id_during_encryption() {
    let key_store = test_key_store();
    let signing_key = test_signing_key();

    let result = encrypt_frame(
        &key_store,
        &signing_key,
        99,
        0,
        0,
        false,
        ENCRYPTION_AES_256_GCM,
        0,
        b"payload",
    );

    assert!(matches!(
        result,
        Err(MoqSecureError::InvalidKeyId(99))
    ));
}

#[test]
fn rejects_unknown_key_id_during_decryption() {
    let key_store = test_key_store();
    let verifying_key = test_verifying_key();

    let signing_key = test_signing_key();

    let frame = encrypt_frame(
        &key_store,
        &signing_key,
        7,
        0,
        0,
        false,
        ENCRYPTION_AES_256_GCM,
        0,
        b"payload",
    )
    .expect("frame should encrypt");

    let mut bytes = frame.serialize();

    // key_id is byte 5 in the fixed header.
    bytes[5] = 99;

    let mut lease_remaining = 0;

    let result = decrypt_frame(
        &key_store,
        &verifying_key,
        &mut lease_remaining,
        &bytes,
    );

    // The modified header is authenticated, but key lookup occurs before
    // AEAD verification.
    assert!(matches!(
        result,
        Err(MoqSecureError::InvalidKeyId(99))
    ));
}

#[test]
fn rejects_tampered_aes256_gcm_ciphertext() {
    let key_store = test_key_store();
    let verifying_key = test_verifying_key();
    let signing_key = test_signing_key();

    let frame = encrypt_frame(
        &key_store,
        &signing_key,
        7,
        42,
        0,
        false,
        ENCRYPTION_AES_256_GCM,
        0,
        b"payload",
    )
    .expect("frame should encrypt");

    let mut bytes = frame.serialize();

    // Header is 17 bytes and the last 16 bytes are the tag.
    bytes[HEADER_LEN] ^= 0x01;

    let mut lease_remaining = 0;

    let result = decrypt_frame(
        &key_store,
        &verifying_key,
        &mut lease_remaining,
        &bytes,
    );

    assert!(matches!(
        result,
        Err(MoqSecureError::AeadAuthFailed)
    ));
}

#[test]
fn rejects_tampered_aes256_gcm_tag() {
    let key_store = test_key_store();
    let verifying_key = test_verifying_key();
    let signing_key = test_signing_key();

    let frame = encrypt_frame(
        &key_store,
        &signing_key,
        7,
        43,
        0,
        false,
        ENCRYPTION_AES_256_GCM,
        0,
        b"payload",
    )
    .expect("frame should encrypt");

    let mut bytes = frame.serialize();
    let last_byte = bytes.len() - 1;
    bytes[last_byte] ^= 0x01;

    let mut lease_remaining = 0;

    let result = decrypt_frame(
        &key_store,
        &verifying_key,
        &mut lease_remaining,
        &bytes,
    );

    assert!(matches!(
        result,
        Err(MoqSecureError::AeadAuthFailed)
    ));
}

#[test]
fn rejects_tampered_aad_header() {
    let key_store = test_key_store();
    let verifying_key = test_verifying_key();
    let signing_key = test_signing_key();

    let frame = encrypt_frame(
        &key_store,
        &signing_key,
        7,
        44,
        0,
        false,
        ENCRYPTION_AES_256_GCM,
        0,
        b"payload",
    )
    .expect("frame should encrypt");

    let mut bytes = frame.serialize();

    // Change the counter without changing ciphertext or tag.
    bytes[13] ^= 0x01;

    let mut lease_remaining = 0;

    let result = decrypt_frame(
        &key_store,
        &verifying_key,
        &mut lease_remaining,
        &bytes,
    );

    assert!(matches!(
        result,
        Err(MoqSecureError::AeadAuthFailed)
    ));
}

#[test]
fn rejects_tampered_signed_frame() {
    let key_store = test_key_store();
    let verifying_key = test_verifying_key();
    let signing_key = test_signing_key();

    let frame = encrypt_frame(
        &key_store,
        &signing_key,
        7,
        45,
        1,
        true,
        ENCRYPTION_AES_256_GCM,
        0,
        b"signed payload",
    )
    .expect("frame should encrypt");

    let mut bytes = frame.serialize();

    // Change the ciphertext while leaving the signature unchanged.
    bytes[HEADER_LEN] ^= 0x01;

    let mut lease_remaining = 0;

    let result = decrypt_frame(
        &key_store,
        &verifying_key,
        &mut lease_remaining,
        &bytes,
    );

    assert!(matches!(
        result,
        Err(MoqSecureError::InvalidSignature)
    ));
}

#[test]
fn signed_frame_consumes_lease_only_after_successful_decryption() {
    let key_store = test_key_store();
    let verifying_key = test_verifying_key();
    let signing_key = test_signing_key();

    let frame = encrypt_frame(
        &key_store,
        &signing_key,
        7,
        46,
        2,
        true,
        ENCRYPTION_AES_256_GCM,
        0,
        b"signed payload",
    )
    .expect("frame should encrypt");

    let mut bytes = frame.serialize();
    bytes[HEADER_LEN] ^= 0x01;

    let mut lease_remaining = 9;

    let result = decrypt_frame(
        &key_store,
        &verifying_key,
        &mut lease_remaining,
        &bytes,
    );

    assert!(matches!(
        result,
        Err(MoqSecureError::InvalidSignature)
    ));

    assert_eq!(
        lease_remaining, 9,
        "failed decryption must not update the lease"
    );
}

#[test]
fn rejects_unsigned_lease_frame_when_lease_is_empty() {
    let key_store = test_key_store();
    let verifying_key = test_verifying_key();
    let signing_key = test_signing_key();

    let frame = encrypt_frame(
        &key_store,
        &signing_key,
        7,
        47,
        1,
        false,
        ENCRYPTION_AES_256_GCM,
        0,
        b"unsigned lease frame",
    )
    .expect("frame should encrypt");

    let mut lease_remaining = 0;

    let result = decrypt_frame(
        &key_store,
        &verifying_key,
        &mut lease_remaining,
        &frame.serialize(),
    );

    assert!(matches!(
        result,
        Err(MoqSecureError::InvalidSignature)
    ));
}
