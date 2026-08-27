use moq_secure::{
    Frame,
    MoqSecureError,
};

const HEADER_LEN: usize = 17;

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
    let mut frame = vec![0u8; HEADER_LEN];
    frame[4] = 1;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::InvalidMagic)
    ));
}

#[test]
fn rejects_invalid_version() {
    let mut frame = vec![0u8; HEADER_LEN];
    frame[0..4].copy_from_slice(b"MOQS");
    frame[4] = 2;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::UnsupportedVersion(2))
    ));
}

#[test]
fn rejects_invalid_flags() {
    let mut frame = vec![0u8; HEADER_LEN];
    frame[0..4].copy_from_slice(b"MOQS");
    frame[4] = 1;

    // Header offsets:
    // n_signed  = 14
    // sig_flag  = 15
    // encrypted = 16
    frame[15] = 2;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::InvalidSigFlag(2))
    ));

    frame[15] = 0;
    frame[16] = 2;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::InvalidEncryptedFlag(2))
    ));
}

#[test]
fn rejects_encrypted_body_without_tag() {
    let mut frame = vec![0u8; HEADER_LEN];
    frame[0..4].copy_from_slice(b"MOQS");
    frame[4] = 1;
    frame[16] = 1;

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::CiphertextTooShort)
    ));
}

#[test]
fn rejects_zero_signature() {
    let mut frame = vec![0u8; HEADER_LEN + 64];
    frame[0..4].copy_from_slice(b"MOQS");
    frame[4] = 1;
    frame[14] = 1; // n_signed
    frame[15] = 1; // sig_flag
    frame[16] = 0; // encrypted

    assert!(matches!(
        Frame::parse(&frame),
        Err(MoqSecureError::InvalidSignature)
    ));
}
