use crate::crypto::{aead_decrypt, aead_encrypt, sha256_digest};
use crate::error::MoqSecureError;

pub const MAGIC: [u8; 4] = *b"MOQS";
pub const VERSION: u8 = 1;

pub const SIG_SLOT_LEN: usize = 64;
pub const AEAD_TAG_LEN: usize = 16;

#[derive(Debug, Clone)]
pub struct WireHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub key_id: u8,
    pub ctr: u64,

    /// Frequency gating parameter:
    /// - n_signed == 0: signing disabled entirely
    /// - n_signed > 0: lease_remaining starts at n_signed; only some frames are signed
    pub n_signed: u8,

    /// Per-frame signed flag (0/1)
    pub sig_flag: u8,

    /// Per-frame encrypted flag (0/1)
    pub encrypted: u8,

    /// Only serialized on the wire when sig_flag == 1
    pub sig_slot: [u8; SIG_SLOT_LEN],
}

impl WireHeader {
    pub fn encode_without_sig_slot(&self) -> Vec<u8> {
        // Fixed AAD/header prefix (sig_slot intentionally omitted)
        // magic(4) + version(1) + key_id(1) + ctr(8) + n_signed(1) + sig_flag(1) + encrypted(1)
        let mut v = Vec::with_capacity(4 + 1 + 1 + 8 + 1 + 1 + 1);
        v.extend_from_slice(&self.magic);
        v.push(self.version);
        v.push(self.key_id);
        v.extend_from_slice(&self.ctr.to_be_bytes());
        v.push(self.n_signed);
        v.push(self.sig_flag);
        v.push(self.encrypted);
        v
    }

    pub fn aad(&self) -> Vec<u8> {
        self.encode_without_sig_slot()
    }
}

#[derive(Debug, Clone)]
pub struct EncryptedFrame {
    pub header: WireHeader,
    pub ciphertext: Vec<u8>,
    pub tag: [u8; AEAD_TAG_LEN],
}

impl EncryptedFrame {
    pub fn serialize(&self) -> Vec<u8> {
        let sig_slot_len_on_wire = if self.header.sig_flag == 1 {
            SIG_SLOT_LEN
        } else {
            0
        };

        let mut out = Vec::with_capacity(
            4 + 1 + 1 + 8 + 1 + 1 + 1
                + sig_slot_len_on_wire
                + self.ciphertext.len()
                + AEAD_TAG_LEN,
        );

        out.extend_from_slice(&self.header.magic);
        out.push(self.header.version);
        out.push(self.header.key_id);
        out.extend_from_slice(&self.header.ctr.to_be_bytes());
        out.push(self.header.n_signed);
        out.push(self.header.sig_flag);
        out.push(self.header.encrypted);

        if self.header.sig_flag == 1 {
            out.extend_from_slice(&self.header.sig_slot);
        }

        out.extend_from_slice(&self.ciphertext);
        out.extend_from_slice(&self.tag);
        out
    }

    pub fn parse(frame: &[u8]) -> Result<Self, MoqSecureError> {
        const FIXED_HEADER_LEN: usize = 4 + 1 + 1 + 8 + 1 + 1 + 1; // 17

        if frame.len() < FIXED_HEADER_LEN + AEAD_TAG_LEN {
            return Err(MoqSecureError::TruncatedFrame);
        }

        let (h_fixed, rest) = frame.split_at(FIXED_HEADER_LEN);
        let mut idx = 0;

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&h_fixed[idx..idx + 4]);
        idx += 4;

        if magic != MAGIC {
            return Err(MoqSecureError::InvalidMagic);
        }

        let version = h_fixed[idx];
        idx += 1;
        if version != VERSION {
            return Err(MoqSecureError::UnsupportedVersion(version));
        }

        let key_id = h_fixed[idx];
        idx += 1;

        let mut ctr_bytes = [0u8; 8];
        ctr_bytes.copy_from_slice(&h_fixed[idx..idx + 8]);
        idx += 8;
        let ctr = u64::from_be_bytes(ctr_bytes);

        let n_signed = h_fixed[idx];
        idx += 1;

        let sig_flag = h_fixed[idx];
        idx += 1;
        if sig_flag != 0 && sig_flag != 1 {
            return Err(MoqSecureError::InvalidSignature);
        }

        let encrypted = h_fixed[idx];
        idx += 1;
        if encrypted != 0 && encrypted != 1 {
            return Err(MoqSecureError::InvalidSignature);
        }

        // sig_slot is only present if sig_flag == 1
        let mut sig_slot = [0u8; SIG_SLOT_LEN];

        let mut ciphertext_and_tag = rest;
        if sig_flag == 1 {
            if ciphertext_and_tag.len() < SIG_SLOT_LEN + AEAD_TAG_LEN {
                return Err(MoqSecureError::TruncatedFrame);
            }
            let (sig_bytes, rem) = ciphertext_and_tag.split_at(SIG_SLOT_LEN);
            sig_slot.copy_from_slice(sig_bytes);
            ciphertext_and_tag = rem;
        }

        // Enforce signature slot rules (when sig_flag==1, sig_slot must be non-zero).
        if sig_flag == 1 && sig_slot == [0u8; SIG_SLOT_LEN] {
            return Err(MoqSecureError::InvalidSignature);
        }

        // If signing disabled entirely, no frame should claim sig_flag==1
        if n_signed == 0 && sig_flag != 0 {
            return Err(MoqSecureError::SigningMismatch);
        }

        if ciphertext_and_tag.len() < AEAD_TAG_LEN {
            return Err(MoqSecureError::CiphertextTooShort);
        }
        let (ciphertext, tag_bytes) = ciphertext_and_tag.split_at(ciphertext_and_tag.len() - AEAD_TAG_LEN);

        let tag: [u8; AEAD_TAG_LEN] = tag_bytes
            .try_into()
            .map_err(|_| MoqSecureError::CiphertextTooShort)?;

        Ok(EncryptedFrame {
            header: WireHeader {
                magic,
                version,
                key_id,
                ctr,
                n_signed,
                sig_flag,
                encrypted,
                sig_slot,
            },
            ciphertext: ciphertext.to_vec(),
            tag,
        })
    }

    pub fn aad_bytes(&self) -> Vec<u8> {
        self.header.aad()
    }

    pub fn digest_for_signature(&self) -> [u8; 32] {
        // digest = SHA256(headerWithoutSigSlot || ciphertext || aeadTag)
        let header_wo_sig = self.header.encode_without_sig_slot();
        let mut v = Vec::with_capacity(header_wo_sig.len() + self.ciphertext.len() + AEAD_TAG_LEN);
        v.extend_from_slice(&header_wo_sig);
        v.extend_from_slice(&self.ciphertext);
        v.extend_from_slice(&self.tag);
        sha256_digest(&v)
    }

    pub fn aead_encrypt_for(&self, _key: &[u8; 32]) -> Result<(), MoqSecureError> {
        Ok(())
    }
}

pub fn encrypt_frame(
    keys: &[[u8; 32]; 256],
    broadcaster_private_key: &ed25519_dalek::SigningKey,
    key_id: u8,
    ctr: u64,
    n_signed: u8,
    maybe_sign: bool,
    plaintext: &[u8],
) -> EncryptedFrame {
    use ed25519_dalek::Signer;

    // sig_flag indicates "this frame carries a signature"
    let sig_flag = if n_signed == 0 {
        0
    } else if maybe_sign {
        1
    } else {
        0
    };

    // This updated code assumes frames are encrypted (encrypted=1).
    // If you need encrypted=0 support (and how to compute ciphertext/tag then),
    // tell me your intended wire behavior and I'll adjust encrypt/decrypt.
    let encrypted = 1u8;

    let mut header = WireHeader {
        magic: MAGIC,
        version: VERSION,
        key_id,
        ctr,
        n_signed,
        sig_flag,
        encrypted,
        sig_slot: [0u8; SIG_SLOT_LEN],
    };

    let aad = header.aad();
    let (ciphertext, tag) = aead_encrypt(
        &keys[key_id as usize],
        key_id,
        ctr,
        &aad,
        plaintext,
    );

    let mut frame = EncryptedFrame {
        header: header.clone(),
        ciphertext,
        tag,
    };

    if n_signed > 0 && sig_flag == 1 {
        let digest = frame.digest_for_signature();
        let sig = broadcaster_private_key.sign(&digest);
        frame.header.sig_slot = sig.to_bytes();
        frame.header.sig_flag = 1;
    } else {
        // No sig_slot is serialized when sig_flag==0; keep it zeroed.
        frame.header.sig_slot = [0u8; SIG_SLOT_LEN];
        frame.header.sig_flag = 0;
    }

    // encrypted flag is already set
    frame
}

pub fn decrypt_frame(
    keys: &[[u8; 32]; 256],
    broadcaster_public_key: &ed25519_dalek::VerifyingKey,
    mut lease_remaining: &mut u8,
    frame_bytes: &[u8],
) -> Result<Vec<u8>, MoqSecureError> {
    use ed25519_dalek::Verifier;

    let frame = EncryptedFrame::parse(frame_bytes)?;

    // ---- strict header/signature invariants ----
    if frame.header.sig_flag != 0 && frame.header.sig_flag != 1 {
        return Err(MoqSecureError::InvalidSignature);
    }
    if frame.header.encrypted != 0 && frame.header.encrypted != 1 {
        return Err(MoqSecureError::InvalidSignature);
    }

    // This updated code assumes we only support encrypted=1 frames.
    if frame.header.encrypted == 0 {
        return Err(MoqSecureError::AeadAuthFailed);
    }

    // signing disabled
    if frame.header.n_signed == 0 {
        if frame.header.sig_flag != 0 || frame.header.sig_slot != [0u8; SIG_SLOT_LEN] {
            return Err(MoqSecureError::SigningMismatch);
        }
    } else {
        // signing enabled: lease gating
        if frame.header.sig_flag == 1 {
            if frame.header.sig_slot == [0u8; SIG_SLOT_LEN] {
                return Err(MoqSecureError::InvalidSignature);
            }

            let digest = frame.digest_for_signature();
            let sig = ed25519_dalek::Signature::from_bytes(&frame.header.sig_slot)
                .map_err(|_| MoqSecureError::InvalidSignature)?;
            broadcaster_public_key
                .verify(&digest, &sig)
                .map_err(|_| MoqSecureError::InvalidSignature)?;

            // Renew lease to accept unsigned frames
            *lease_remaining = frame.header.n_signed;
        } else {
            // unsigned frame: lease gating
            if *lease_remaining == 0 {
                return Err(MoqSecureError::InvalidSignature);
            }
            *lease_remaining -= 1;
        }
    }

    // ---- AEAD decrypt ----
    let aad = frame.aad_bytes();
    aead_decrypt(
        &keys[frame.header.key_id as usize],
        frame.header.key_id,
        frame.header.ctr,
        &aad,
        &frame.ciphertext,
        &frame.tag,
    )
    .map_err(|_| MoqSecureError::AeadAuthFailed)
}
