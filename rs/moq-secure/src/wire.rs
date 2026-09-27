use crate::crypto::{
    aead_decrypt, aead_encrypt, sha256_digest, AEAD_TAG_LEN,
};
use crate::error::MoqSecureError;
use crate::key_store::KeyStore;
use ed25519_dalek::{Signer, Verifier};

pub const MAGIC: [u8; 4] = *b"MOQS";
pub const VERSION: u8 = 1;

pub const SIG_SLOT_LEN: usize = 64;
pub const PAD_LEN_FIELD_LEN: usize = 4;

pub const ENCRYPTION_UNENCRYPTED: u8 = 0;
pub const ENCRYPTION_CHACHA20_POLY1305: u8 = 1;
pub const ENCRYPTION_AES_256_GCM: u8 = 2;

// magic(4) | version(1) | key_id(1) | ctr(8) |
// n_signed(1) | sig_flag(1) | encryption_type(1)
pub const FIXED_HEADER_LEN: usize = 4 + 1 + 1 + 8 + 1 + 1 + 1;

fn is_valid_encryption_type(value: u8) -> bool {
    matches!(
        value,
        ENCRYPTION_UNENCRYPTED
            | ENCRYPTION_CHACHA20_POLY1305
            | ENCRYPTION_AES_256_GCM
    )
}

fn is_encrypted(encryption_type: u8) -> bool {
    encryption_type != ENCRYPTION_UNENCRYPTED
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EncryptionType {
    Unencrypted = ENCRYPTION_UNENCRYPTED,
    ChaCha20Poly1305 = ENCRYPTION_CHACHA20_POLY1305,
    Aes256Gcm = ENCRYPTION_AES_256_GCM,
}

impl TryFrom<u8> for EncryptionType {
    type Error = MoqSecureError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            ENCRYPTION_UNENCRYPTED => Ok(Self::Unencrypted),
            ENCRYPTION_CHACHA20_POLY1305 => {
                Ok(Self::ChaCha20Poly1305)
            }
            ENCRYPTION_AES_256_GCM => Ok(Self::Aes256Gcm),
            value => Err(MoqSecureError::UnsupportedAlgorithm(value)),
        }
    }
}

impl From<EncryptionType> for u8 {
    fn from(value: EncryptionType) -> Self {
        value as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub key_id: u8,
    pub ctr: u64,
    pub n_signed: u8,
    pub sig_flag: u8,
    pub encryption_type: u8,
}

impl WireHeader {
    pub fn encode(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(FIXED_HEADER_LEN);

        result.extend_from_slice(&self.magic);
        result.push(self.version);
        result.push(self.key_id);
        result.extend_from_slice(&self.ctr.to_be_bytes());
        result.push(self.n_signed);
        result.push(self.sig_flag);
        result.push(self.encryption_type);

        result
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

        if !is_valid_encryption_type(self.encryption_type) {
            return Err(MoqSecureError::UnsupportedAlgorithm(
                self.encryption_type,
            ));
        }

        if self.n_signed == 0 && self.sig_flag != 0 {
            return Err(MoqSecureError::SigningMismatch);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    /// For encrypted frames, this is ciphertext without the AEAD tag.
    ///
    /// For unencrypted frames, this is:
    ///
    /// `pad_len (4-byte big endian) || padding || plaintext`
    pub header: WireHeader,
    pub payload: Vec<u8>,
    pub tag: [u8; AEAD_TAG_LEN],
    pub signature: Option<[u8; SIG_SLOT_LEN]>,
}

impl Frame {
    pub fn parse(frame: &[u8]) -> Result<Self, MoqSecureError> {
        if frame.len() < FIXED_HEADER_LEN {
            return Err(MoqSecureError::TruncatedFrame);
        }

        let (header_bytes, rest) = frame.split_at(FIXED_HEADER_LEN);

        let mut offset = 0;

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&header_bytes[offset..offset + 4]);
        offset += 4;

        let version = header_bytes[offset];
        offset += 1;

        let key_id = header_bytes[offset];
        offset += 1;

        let mut ctr_bytes = [0u8; 8];
        ctr_bytes.copy_from_slice(&header_bytes[offset..offset + 8]);
        offset += 8;

        let ctr = u64::from_be_bytes(ctr_bytes);

        let n_signed = header_bytes[offset];
        offset += 1;

        let sig_flag = header_bytes[offset];
        offset += 1;

        let encryption_type = header_bytes[offset];

        let header = WireHeader {
            magic,
            version,
            key_id,
            ctr,
            n_signed,
            sig_flag,
            encryption_type,
        };

        header.validate()?;

        let signature_len = if header.sig_flag == 1 {
            SIG_SLOT_LEN
        } else {
            0
        };

        if rest.len() < signature_len {
            return Err(MoqSecureError::TruncatedFrame);
        }

        let (body, signature_bytes) = if signature_len == 0 {
            (rest, None)
        } else {
            let body_end = rest.len() - SIG_SLOT_LEN;
            let (body, signature) = rest.split_at(body_end);
            (body, Some(signature))
        };

        let signature = match signature_bytes {
            None => None,
            Some(bytes) => {
                if bytes.len() != SIG_SLOT_LEN {
                    return Err(MoqSecureError::TruncatedFrame);
                }

                let mut signature = [0u8; SIG_SLOT_LEN];
                signature.copy_from_slice(bytes);

                if signature == [0u8; SIG_SLOT_LEN] {
                    return Err(MoqSecureError::InvalidSignature);
                }

                Some(signature)
            }
        };

        if is_encrypted(header.encryption_type) {
            if body.len() < AEAD_TAG_LEN {
                return Err(MoqSecureError::CiphertextTooShort);
            }

            let ciphertext_len = body.len() - AEAD_TAG_LEN;
            let ciphertext = &body[..ciphertext_len];
            let tag_bytes = &body[ciphertext_len..];

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
                payload: body.to_vec(),
                tag: [0u8; AEAD_TAG_LEN],
                signature,
            })
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let encrypted = is_encrypted(self.header.encryption_type);

        let signature_len = if self.header.sig_flag == 1 {
            SIG_SLOT_LEN
        } else {
            0
        };

        let body_len = self.payload.len()
            + if encrypted { AEAD_TAG_LEN } else { 0 };

        let mut result = Vec::with_capacity(
            FIXED_HEADER_LEN + body_len + signature_len,
        );

        result.extend_from_slice(&self.header.encode());
        result.extend_from_slice(&self.payload);

        if encrypted {
            result.extend_from_slice(&self.tag);
        }

        if self.header.sig_flag == 1 {
            if let Some(signature) = self.signature {
                result.extend_from_slice(&signature);
            } else {
                result.extend_from_slice(&[0u8; SIG_SLOT_LEN]);
            }
        }

        result
    }

    pub fn aad_bytes(&self) -> Vec<u8> {
        self.header.aad()
    }

    pub fn digest_for_signature(&self) -> [u8; 32] {
        let header = self.header.encode();
        let encrypted = is_encrypted(self.header.encryption_type);

        let mut data = Vec::with_capacity(
            header.len()
                + self.payload.len()
                + if encrypted { AEAD_TAG_LEN } else { 0 },
        );

        data.extend_from_slice(&header);
        data.extend_from_slice(&self.payload);

        if encrypted {
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
        let signing_enabled = self.header.n_signed > 0;
        let signed = self.header.sig_flag == 1;

        /*
         * Verify the signing state before decrypting. The lease is committed
         * only after decryption and padding validation succeed.
         */
        let next_lease_remaining = if !signing_enabled {
            if self.header.sig_flag != 0 || self.signature.is_some() {
                return Err(MoqSecureError::SigningMismatch);
            }

            None
        } else if signed {
            let signature_bytes = self
                .signature
                .ok_or(MoqSecureError::InvalidSignature)?;

            let signature =
                ed25519_dalek::Signature::from_bytes(&signature_bytes);

            let digest = self.digest_for_signature();

            broadcaster_public_key
                .verify(&digest, &signature)
                .map_err(|_| MoqSecureError::InvalidSignature)?;

            Some(self.header.n_signed.saturating_sub(1))
        } else {
            if *lease_remaining == 0 {
                return Err(MoqSecureError::InvalidSignature);
            }

            Some(lease_remaining.saturating_sub(1))
        };

        let padded_plaintext = if is_encrypted(self.header.encryption_type) {
            let key = key_store
                .aead_key(self.header.key_id)
                .ok_or(MoqSecureError::InvalidKeyId(self.header.key_id))?;

            aead_decrypt(
                self.header.encryption_type,
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
        pad_len_bytes.copy_from_slice(
            &padded_plaintext[..PAD_LEN_FIELD_LEN],
        );

        let pad_len = u32::from_be_bytes(pad_len_bytes) as usize;

        let content_start = PAD_LEN_FIELD_LEN
            .checked_add(pad_len)
            .ok_or(MoqSecureError::InvalidPadLength)?;

        if content_start > padded_plaintext.len() {
            return Err(MoqSecureError::InvalidPadLength);
        }

        let plaintext = padded_plaintext[content_start..].to_vec();

        if let Some(next) = next_lease_remaining {
            *lease_remaining = next;
        }

        Ok(plaintext)
    }
}

pub fn encrypt_frame(
    key_store: &dyn KeyStore,
    broadcaster_private_key: &ed25519_dalek::SigningKey,
    key_id: u8,
    ctr: u64,
    n_signed: u8,
    maybe_sign: bool,
    encryption_type: u8,
    pad_len: u32,
    plaintext: &[u8],
) -> Result<Frame, MoqSecureError> {
    if !is_valid_encryption_type(encryption_type) {
        return Err(MoqSecureError::UnsupportedAlgorithm(
            encryption_type,
        ));
    }

    let sig_flag = if n_signed != 0 && maybe_sign {
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
        encryption_type,
    };

    header.validate()?;

    let pad_len_usize = pad_len as usize;

    let mut padded_plaintext = Vec::with_capacity(
        PAD_LEN_FIELD_LEN + pad_len_usize + plaintext.len(),
    );

    padded_plaintext.extend_from_slice(&pad_len.to_be_bytes());
    padded_plaintext.resize(
        PAD_LEN_FIELD_LEN + pad_len_usize,
        0,
    );
    padded_plaintext.extend_from_slice(plaintext);

    let frame_without_signature =
        if is_encrypted(encryption_type) {
            let key = key_store
                .aead_key(key_id)
                .ok_or(MoqSecureError::InvalidKeyId(key_id))?;

            let (ciphertext, tag) = aead_encrypt(
                encryption_type,
                key,
                key_id,
                ctr,
                &header.aad(),
                &padded_plaintext,
            )?;

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

