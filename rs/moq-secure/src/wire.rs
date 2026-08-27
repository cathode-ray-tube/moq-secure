use crate::crypto::{aead_decrypt, aead_encrypt, sha256_digest};
use crate::error::MoqSecureError;
use crate::key_store::KeyStore;
use ed25519_dalek::{Signer, Verifier};

pub const MAGIC: [u8; 4] = *b"MOQS";
pub const VERSION: u8 = 1;

pub const SIG_SLOT_LEN: usize = 64;
pub const AEAD_TAG_LEN: usize = 16;
pub const PAD_LEN_FIELD_LEN: usize = 4;

// magic(4) | version(1) | key_id(1) | ctr(8) |
// n_signed(1) | sig_flag(1) | encrypted(1)
pub const FIXED_HEADER_LEN: usize = 4 + 1 + 1 + 8 + 1 + 1 + 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub key_id: u8,
    pub ctr: u64,
    pub n_signed: u8,
    pub sig_flag: u8,
    pub encrypted: u8,
}

impl WireHeader {
    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(FIXED_HEADER_LEN);

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
            return Err(MoqSecureError::InvalidSigFlag(self.sig_flag));
        }

        if self.encrypted != 0 && self.encrypted != 1 {
            return Err(MoqSecureError::InvalidEncryptedFlag(self.encrypted));
        }

        if self.n_signed == 0 && self.sig_flag != 0 {
            return Err(MoqSecureError::SigningMismatch);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub header: WireHeader,

    // For encrypted frames, payload is ciphertext without the AEAD tag.
    // For unencrypted frames, payload is:
    // padLen(4-byte big endian) || padding || plaintext.
    pub payload: Vec<u8>,

    pub tag: [u8; AEAD_TAG_LEN],
    pub signature: Option<[u8; SIG_SLOT_LEN]>,
}

impl Frame {
    pub fn parse(frame: &[u8]) -> Result<Self, MoqSecureError> {
        if frame.len() < FIXED_HEADER_LEN {
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

        let header = WireHeader {
            magic,
            version,
            key_id,
            ctr,
            n_signed,
            sig_flag,
            encrypted,
        };

        header.validate()?;

        let sig_len = if header.sig_flag == 1 {
            SIG_SLOT_LEN
        } else {
            0
        };

        if rest0.len() < sig_len {
            return Err(MoqSecureError::TruncatedFrame);
        }

        let (payload_and_tag, signature_bytes) = if sig_len == 0 {
            (rest0, None)
        } else {
            let split_at = rest0.len() - SIG_SLOT_LEN;
            let (payload, signature) = rest0.split_at(split_at);
            (payload, Some(signature))
        };

        let signature = if let Some(sig_bytes) = signature_bytes {
            if sig_bytes.len() != SIG_SLOT_LEN {
                return Err(MoqSecureError::TruncatedFrame);
            }

            let mut sig = [0u8; SIG_SLOT_LEN];
            sig.copy_from_slice(sig_bytes);

            if sig == [0u8; SIG_SLOT_LEN] {
                return Err(MoqSecureError::InvalidSignature);
            }

            Some(sig)
        } else {
            None
        };

        if header.encrypted == 1 {
            if payload_and_tag.len() < AEAD_TAG_LEN {
                return Err(MoqSecureError::CiphertextTooShort);
            }

            let split_at = payload_and_tag.len() - AEAD_TAG_LEN;
            let ciphertext = &payload_and_tag[..split_at];
            let tag_bytes = &payload_and_tag[split_at..];

            let tag: [u8; AEAD_TAG_LEN] = tag_bytes
                .try_into()
                .map_err(|_| MoqSecureError::CiphertextTooShort)?;

            Ok(Self {
                header,
                payload: ciphertext.to_vec(),
                tag,
                signature,
            })
        } else {
            Ok(Self {
                header,
                payload: payload_and_tag.to_vec(),
                tag: [0u8; AEAD_TAG_LEN],
                signature,
            })
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();

        out.extend_from_slice(&self.header.encode());

        if self.header.encrypted == 1 {
            out.extend_from_slice(&self.payload);
            out.extend_from_slice(&self.tag);
        } else {
            out.extend_from_slice(&self.payload);
        }

        if self.header.sig_flag == 1 {
            if let Some(signature) = self.signature {
                out.extend_from_slice(&signature);
            } else {
                out.extend_from_slice(&[0u8; SIG_SLOT_LEN]);
            }
        }

        out
    }

    pub fn aad_bytes(&self) -> Vec<u8> {
        self.header.aad()
    }

    pub fn digest_for_signature(&self) -> [u8; 32] {
        let header_bytes = self.header.encode();

        let mut data = Vec::with_capacity(
            header_bytes.len()
                + self.payload.len()
                + if self.header.encrypted == 1 {
                    AEAD_TAG_LEN
                } else {
                    0
                },
        );

        data.extend_from_slice(&header_bytes);
        data.extend_from_slice(&self.payload);

        if self.header.encrypted == 1 {
            data.extend_from_slice(&self.tag);
        }

        sha256_digest(&data)
    }

    pub fn decode_plaintext_with_key_store(
        &self,
        key_store: &dyn KeyStore,
        broadcaster_public_key: &ed25519_dalek::VerifyingKey,
        lease_remaining: &mut u8,
    ) -> Result<Vec<u8>, MoqSecureError> {
        if self.header.n_signed == 0 {
            if self.header.sig_flag != 0 || self.signature.is_some() {
                return Err(MoqSecureError::SigningMismatch);
            }
        } else if self.header.sig_flag == 1 {
            let signature_bytes = self
                .signature
                .ok_or(MoqSecureError::InvalidSignature)?;

            let signature =
                ed25519_dalek::Signature::from_bytes(&signature_bytes);

            let digest = self.digest_for_signature();

            broadcaster_public_key
                .verify(&digest, &signature)
                .map_err(|_| MoqSecureError::InvalidSignature)?;

            *lease_remaining = self.header.n_signed;
        } else {
            if *lease_remaining == 0 {
                return Err(MoqSecureError::InvalidSignature);
            }

            *lease_remaining -= 1;
        }

        let padded_plaintext = if self.header.encrypted == 1 {
            let key = key_store
                .aead_key(self.header.key_id)
                .ok_or(MoqSecureError::InvalidKeyId(self.header.key_id))?;

            aead_decrypt(
                key,
                self.header.key_id,
                self.header.ctr,
                &self.aad_bytes(),
                &self.payload,
                &self.tag,
            )?
        } else {
            self.payload.clone()
        };

        if padded_plaintext.len() < PAD_LEN_FIELD_LEN {
            return Err(MoqSecureError::InvalidPadLength);
        }

        let mut pad_len_bytes = [0u8; PAD_LEN_FIELD_LEN];
        pad_len_bytes.copy_from_slice(&padded_plaintext[..PAD_LEN_FIELD_LEN]);

        let pad_len = u32::from_be_bytes(pad_len_bytes) as usize;
        let content_start = PAD_LEN_FIELD_LEN
            .checked_add(pad_len)
            .ok_or(MoqSecureError::InvalidPadLength)?;

        if content_start > padded_plaintext.len() {
            return Err(MoqSecureError::InvalidPadLength);
        }

        Ok(padded_plaintext[content_start..].to_vec())
    }
}

pub fn encrypt_frame(
    key_store: &dyn KeyStore,
    broadcaster_private_key: &ed25519_dalek::SigningKey,
    key_id: u8,
    ctr: u64,
    n_signed: u8,
    maybe_sign: bool,
    encrypted: u8,
    pad_len: u32,
    plaintext: &[u8],
) -> Result<Frame, MoqSecureError> {
    if encrypted != 0 && encrypted != 1 {
        return Err(MoqSecureError::InvalidEncryptedFlag(encrypted));
    }

    let sig_flag = if n_signed != 0 && maybe_sign { 1 } else { 0 };

    let header = WireHeader {
        magic: MAGIC,
        version: VERSION,
        key_id,
        ctr,
        n_signed,
        sig_flag,
        encrypted,
    };

    header.validate()?;

    let pad_len_usize = pad_len as usize;
    let mut padded_plaintext = Vec::with_capacity(
        PAD_LEN_FIELD_LEN + pad_len_usize + plaintext.len(),
    );

    padded_plaintext.extend_from_slice(&pad_len.to_be_bytes());
    padded_plaintext.resize(PAD_LEN_FIELD_LEN + pad_len_usize, 0);
    padded_plaintext.extend_from_slice(plaintext);

    let frame_without_signature = if encrypted == 1 {
        let key = key_store
            .aead_key(key_id)
            .ok_or(MoqSecureError::InvalidKeyId(key_id))?;

        let (ciphertext, tag) = aead_encrypt(
            key,
            key_id,
            ctr,
            &header.aad(),
            &padded_plaintext,
        );

        Frame {
            header,
            payload: ciphertext,
            tag,
            signature: None,
        }
    } else {
        Frame {
            header,
            payload: padded_plaintext,
            tag: [0u8; AEAD_TAG_LEN],
            signature: None,
        }
    };

    if sig_flag == 1 {
        let digest = frame_without_signature.digest_for_signature();
        let signature = broadcaster_private_key.sign(&digest);

        Ok(Frame {
            signature: Some(signature.to_bytes()),
            ..frame_without_signature
        })
    } else {
        Ok(frame_without_signature)
    }
}

pub fn decrypt_frame(
    key_store: &dyn KeyStore,
    broadcaster_public_key: &ed25519_dalek::VerifyingKey,
    lease_remaining: &mut u8,
    frame_bytes: &[u8],
) -> Result<Vec<u8>, MoqSecureError> {
    let frame = Frame::parse(frame_bytes)?;

    frame.decode_plaintext_with_key_store(
        key_store,
        broadcaster_public_key,
        lease_remaining,
    )
}
