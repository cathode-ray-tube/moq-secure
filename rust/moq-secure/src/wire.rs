// wire.rs  (wire/frame implementation)
//
// This module implements the NEW per-frame wire format and signing/encryption logic.

use crate::crypto::{aead_decrypt, aead_encrypt, sha256_digest};
use crate::error::MoqSecureError;

pub const MAGIC: [u8; 4] = *b"MOQS";
pub const VERSION: u8 = 1;

pub const SIG_SLOT_LEN: usize = 64; // signature trailer length
pub const AEAD_TAG_LEN: usize = 16;

pub const FIXED_HEADER_LEN: usize = 4 + 1 + 1 + 8 + 1 + 1 + 1 + 4; // 28

#[derive(Debug, Clone)]
pub struct WireHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub key_id: u8,
    pub ctr: u64,

    pub n_signed: u8,
    pub sig_flag: u8,
    pub encrypted: u8,

    // padLen: number of zero bytes prepended to plaintext before encryption
    // (or appended to plaintext payload when encrypted==0)
    pub pad_len: u32,
}

impl WireHeader {
    pub fn encode(&self) -> Vec<u8> {
        // magic(4) | version(1) | keyId(1) | ctr(8) | nSigned(1) | sigFlag(1) | encrypted(1) | padLen(4)
        let mut v = Vec::with_capacity(FIXED_HEADER_LEN);
        v.extend_from_slice(&self.magic);
        v.push(self.version);
        v.push(self.key_id);
        v.extend_from_slice(&self.ctr.to_be_bytes());
        v.push(self.n_signed);
        v.push(self.sig_flag);
        v.push(self.encrypted);
        v.extend_from_slice(&self.pad_len.to_be_bytes());
        v
    }

    pub fn aad(&self) -> Vec<u8> {
        // Signature bytes are never included in AEAD AAD.
        // AAD is exactly the unencrypted header bytes.
        self.encode()
    }

    pub fn validate(&self) -> Result<(), MoqSecureError> {
        if self.magic != MAGIC {
            return Err(MoqSecureError::InvalidMagic);
        }
        if self.version != VERSION {
            return Err(MoqSecureError::UnsupportedVersion(self.version));
        }
        if self.sig_flag != 0 && self.sig_flag != 1 {
            return Err(MoqSecureError::InvalidSignature);
        }
        if self.encrypted != 0 && self.encrypted != 1 {
            return Err(MoqSecureError::InvalidSignature);
        }
        // If n_signed==0, then sig_flag must be 0 (signing disabled entirely).
        if self.n_signed == 0 && self.sig_flag != 0 {
            return Err(MoqSecureError::SigningMismatch);
        }
        Ok(())
    }

    pub fn pad_len_usize(&self) -> usize {
        self.pad_len as usize
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub header: WireHeader,

    // If encrypted==1: payload bytes = ciphertext (N) and tag is present
    // If encrypted==0: payload bytes = plaintext (N) and tag is unused/zero
    pub payload: Vec<u8>,
    pub tag: [u8; AEAD_TAG_LEN],

    // If sig_flag==1: signature trailer is present as last 64 bytes of the frame.
    pub signature: Option<[u8; SIG_SLOT_LEN]>,
}

impl Frame {
    pub fn parse(frame: &[u8]) -> Result<Self, MoqSecureError> {
        if frame.len() < FIXED_HEADER_LEN + AEAD_TAG_LEN {
            // minimum could be smaller when encrypted==0 (no tag), but we’ll do stricter below
            return Err(MoqSecureError::TruncatedFrame);
        }

        let (h_bytes, rest0) = frame.split_at(FIXED_HEADER_LEN);

        let mut idx = 0;

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&h_bytes[idx..idx + 4]);
        idx += 4;

        let version = h_bytes[idx];
        idx += 1;

        let key_id = h_bytes[idx];
        idx += 1;

        let mut ctr_bytes = [0u8; 8];
        ctr_bytes.copy_from_slice(&h_bytes[idx..idx + 8]);
        idx += 8;
        let ctr = u64::from_be_bytes(ctr_bytes);

        let n_signed = h_bytes[idx];
        idx += 1;

        let sig_flag = h_bytes[idx];
        idx += 1;

        let encrypted = h_bytes[idx];
        idx += 1;

        let mut pad_len_bytes = [0u8; 4];
        pad_len_bytes.copy_from_slice(&h_bytes[idx..idx + 4]);
        idx += 4;
        let pad_len = u32::from_be_bytes(pad_len_bytes);

        let header = WireHeader {
            magic,
            version,
            key_id,
            ctr,
            n_signed,
            sig_flag,
            encrypted,
            pad_len,
        };
        header.validate()?;

        // Trailer signature length depends only on sig_flag.
        let sig_len_on_wire = if header.sig_flag == 1 {
            SIG_SLOT_LEN
        } else {
            0
        };

        if rest0.len() < sig_len_on_wire {
            return Err(MoqSecureError::TruncatedFrame);
        }

        let (payload_and_optional_tag, sig_opt) = if sig_len_on_wire == 0 {
            (rest0, None)
        } else {
            let (p, s) = rest0.split_at(rest0.len() - SIG_SLOT_LEN);
            (p, Some(s))
        };

        let signature = if let Some(sig_bytes) = sig_opt {
            if sig_bytes.len() != SIG_SLOT_LEN {
                return Err(MoqSecureError::TruncatedFrame);
            }
            let mut sig = [0u8; SIG_SLOT_LEN];
            sig.copy_from_slice(sig_bytes);
            // Optional strictness: non-zero signature when present
            if sig == [0u8; SIG_SLOT_LEN] {
                return Err(MoqSecureError::InvalidSignature);
            }
            Some(sig)
        } else {
            if header.sig_flag == 1 {
                return Err(MoqSecureError::TruncatedFrame);
            }
            None
        };

        if header.encrypted == 1 {
            if payload_and_optional_tag.len() < AEAD_TAG_LEN {
                return Err(MoqSecureError::CiphertextTooShort);
            }
            let (ciphertext, tag_bytes) =
                payload_and_optional_tag.split_at(payload_and_optional_tag.len() - AEAD_TAG_LEN);

            let tag: [u8; AEAD_TAG_LEN] = tag_bytes.try_into().map_err(|_| MoqSecureError::CiphertextTooShort)?;

            Ok(Frame {
                header,
                payload: ciphertext.to_vec(),
                tag,
                signature,
            })
        } else {
            // encrypted==0: plaintext only, no AEAD tag on wire
            Ok(Frame {
                header,
                payload: payload_and_optional_tag.to_vec(),
                tag: [0u8; AEAD_TAG_LEN],
                signature,
            })
        }
    }

    pub fn aad_bytes(&self) -> Vec<u8> {
        self.header.aad()
    }

    // Signature preimage:
    // sha256(header_wo_sigslot || (ciphertext||tag if encrypted==1 else plaintext) )
    pub fn digest_for_signature(&self) -> [u8; 32] {
        let header_bytes = self.header.encode(); // header without any signature bytes

        if self.header.encrypted == 1 {
            let mut v = Vec::with_capacity(
                header_bytes.len() + self.payload.len() + AEAD_TAG_LEN,
            );
            v.extend_from_slice(&header_bytes);
            v.extend_from_slice(&self.payload); // ciphertext
            v.extend_from_slice(&self.tag);     // aead tag
            sha256_digest(&v)
        } else {
            let mut v = Vec::with_capacity(header_bytes.len() + self.payload.len());
            v.extend_from_slice(&header_bytes);
            v.extend_from_slice(&self.payload); // plaintext (includes zero prefix)
            sha256_digest(&v)
        }
    }

    pub fn decode_plaintext(&self) -> Result<Vec<u8>, MoqSecureError> {
        // First obtain "padded_plaintext"
        let padded_plain = if self.header.encrypted == 1 {
            aead_decrypt(
                // key
                &crate::crypto_key_table()[self.header.key_id as usize],
                self.header.key_id,
                self.header.ctr,
                &self.header.aad(),
                &self.payload,      // ciphertext
                &self.tag,
            )?
        } else {
            self.payload.clone()
        };

        let pad_len = self.header.pad_len_usize();
        if padded_plain.len() < pad_len {
            return Err(MoqSecureError::AeadAuthFailed);
        }

        Ok(padded_plain[pad_len..].to_vec())
    }
}


pub fn encrypt_frame(
    keys: &[[u8; 32]; 256],
    broadcaster_private_key: &ed25519_dalek::SigningKey,
    key_id: u8,
    ctr: u64,
    n_signed: u8,
    maybe_sign: bool,
    encrypted: u8, // 0 or 1
    pad_len: u32,
    plaintext: &[u8],
) -> Result<Frame, MoqSecureError> {
    use ed25519_dalek::Signer;

    if encrypted != 0 && encrypted != 1 {
        return Err(MoqSecureError::InvalidSignature);
    }

    let sig_flag = if n_signed == 0 {
        0
    } else if maybe_sign {
        1
    } else {
        0
    };

    let header = WireHeader {
        magic: MAGIC,
        version: VERSION,
        key_id,
        ctr,
        n_signed,
        sig_flag,
        encrypted,
        pad_len,
    };
    header.validate()?;

    // padded_plaintext = padLen zero bytes || plaintext
    let pad_len_usize = pad_len as usize;
    let mut padded_plaintext = Vec::with_capacity(pad_len_usize + plaintext.len());
    padded_plaintext.extend(std::iter::repeat(0u8).take(pad_len_usize));
    padded_plaintext.extend_from_slice(plaintext);

    let aad = header.aad();

    if encrypted == 1 {
        let (ciphertext, tag) = aead_encrypt(
            &keys[key_id as usize],
            key_id,
            ctr,
            &aad,
            &padded_plaintext,
        );

        let frame_wo_sig = Frame {
            header: header.clone(),
            payload: ciphertext,
            tag,
            signature: None,
        };

        if sig_flag == 1 {
            let digest = frame_wo_sig.digest_for_signature();
            let sig = broadcaster_private_key.sign(&digest);
            Ok(Frame {
                signature: Some(sig.to_bytes()),
                ..frame_wo_sig
            })
        } else {
            Ok(Frame {
                signature: None,
                ..frame_wo_sig
            })
        }
    } else {
        // encrypted==0: plaintext is written directly into payload, no AEAD tag
        let frame_wo_sig = Frame {
            header: header.clone(),
            payload: padded_plaintext,
            tag: [0u8; AEAD_TAG_LEN],
            signature: None,
        };

        if sig_flag == 1 {
            let digest = frame_wo_sig.digest_for_signature();
            let sig = broadcaster_private_key.sign(&digest);
            Ok(Frame {
                signature: Some(sig.to_bytes()),
                ..frame_wo_sig
            })
        } else {
            Ok(Frame {
                signature: None,
                ..frame_wo_sig
            })
        }
    }
}

pub fn decrypt_frame(
    keys: &[[u8; 32]; 256],
    broadcaster_public_key: &ed25519_dalek::VerifyingKey,
    mut lease_remaining: &mut u8,
    frame_bytes: &[u8],
) -> Result<Vec<u8>, MoqSecureError> {
    use ed25519_dalek::Verifier;

    let frame = Frame::parse(frame_bytes)?;

    // Lease/signing gating (your existing logic)
    if frame.header.n_signed == 0 {
        if frame.header.sig_flag != 0 || frame.signature.is_some() {
            return Err(MoqSecureError::SigningMismatch);
        }
    } else {
        if frame.header.sig_flag == 1 {
            let sig_bytes = frame.signature.ok_or(MoqSecureError::InvalidSignature)?;
            let digest = frame.digest_for_signature();
            let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes)
                .map_err(|_| MoqSecureError::InvalidSignature)?;
            broadcaster_public_key
                .verify(&digest, &sig)
                .map_err(|_| MoqSecureError::InvalidSignature)?;

            *lease_remaining = frame.header.n_signed;
        } else {
            if *lease_remaining == 0 {
                return Err(MoqSecureError::InvalidSignature);
            }
            *lease_remaining -= 1;
        }
    }

    // Decrypt/pass-through to get padded_plaintext
    let padded_plaintext = if frame.header.encrypted == 1 {
        aead_decrypt(
            &keys[frame.header.key_id as usize],
            frame.header.key_id,
            frame.header.ctr,
            &frame.aad_bytes(),
            &frame.payload,
            &frame.tag,
        )?
    } else {
        frame.payload.clone()
    };

    // Strip padLen zero prefix
    let pad_len = frame.header.pad_len_usize();
    if padded_plaintext.len() < pad_len {
        return Err(MoqSecureError::AeadAuthFailed);
    }

    Ok(padded_plaintext[pad_len..].to_vec())
}
