use moq_secure::wire::{decrypt_frame, encrypt_frame};
use crate::keys::ChatKeys;
use anyhow::Context;

pub struct ChatSession {
    pub keys: ChatKeys,
    pub broadcast: String,
    pub track: String,
}

impl ChatSession {
    pub fn new(keys: ChatKeys, broadcast: String, track: String) -> Self {
        Self { keys, broadcast, track }
    }

    fn keystore_array(&self) -> [[u8; 32]; 256] {
        let mut ks = [[0u8; 32]; 256];
        ks[self.keys.key_id as usize] = self.keys.aead_key;
        ks
    }
}

/// Publisher publishes each chat message as one MoQ group containing one frame.
/// This is the “easy mapping”: 1 chat msg == 1 group with one object.
pub struct ChatPublisher {
    pub keys: ChatKeys,
    pub track: moq_net::track::Producer,
    pub n_signed: u8, // 0 disables signing; CLI will use 1
    pub ctr: u64,
}

impl ChatPublisher {
    pub fn new(track: moq_net::track::Producer, keys: ChatKeys) -> Self {
        // CLI requirement: every message encrypted + signed => sig enabled for all messages.
        Self {
            keys,
            track,
            n_signed: 1,
            ctr: 0,
        }
    }

    pub async fn send_message(&mut self, plaintext: &[u8]) -> anyhow::Result<()> {
        let ctr = self.ctr;
        self.ctr = self.ctr.wrapping_add(1);

        // pad_len=0 for simplicity; you can add options later.
        let pad_len = 0u32;

        // encrypted=1 (your cli requirement implies every message encrypted)
        // maybe_sign=true (sig_flag=1 when n_signed>0)
        let frame = encrypt_frame(
            &self.keystore_array(),
            &self.keys.signing_private,
            self.keys.key_id,
            ctr,
            self.n_signed,
            true,
            1,
            pad_len,
            plaintext,
        )
        .context("encrypt_frame failed")?;

        let frame_bytes = frame.serialize();

        let mut group = self.track.create_group(0u64.into()).context("create_group")?;
        // One object per group.
        group.write_frame(moq_native::moq_net::Timestamp::now(), frame_bytes)?;

        group.finish()?;

        Ok(())
    }

    fn keystore_array(&self) -> [[u8; 32]; 256] {
        let mut ks = [[0u8; 32]; 256];
        ks[self.keys.key_id as usize] = self.keys.aead_key;
        ks
    }

    pub fn finish(self) {
        // Dropping producer usually closes; caller may also close broadcast externally.
        drop(self.track);
    }
}

/// Subscriber receives chat frames, verifies signature + decrypts.
pub struct ChatSubscriber {
    pub verify_key: ed25519_dalek::VerifyingKey,
    pub keys: ChatKeys, // holds aead key too
    pub track: moq_net::track::Subscriber,
    pub lease_remaining: u8,
}

impl ChatSubscriber {
    pub fn new(track: moq_net::track::Subscriber, keys: ChatKeys) -> Self {
        Self {
            verify_key: keys.signing_verify,
            keys,
            track,
            lease_remaining: 0,
        }
    }

    pub async fn run(mut self, mut on_message: impl FnMut(Vec<u8>) + Send) -> anyhow::Result<()> {
        while let Some(mut group) = self.track.recv_group().await? {
            while let Some(object) = group.read_frame().await? {
                let frame_bytes = &object.payload;

                let plaintext = decrypt_frame(
                    &self.keystore_array(),
                    &self.verify_key,
                    &mut self.lease_remaining,
                    frame_bytes,
                )
                .context("decrypt_frame failed")?;

                on_message(plaintext);
            }
        }
        Ok(())
    }

    fn keystore_array(&self) -> [[u8; 32]; 256] {
        let mut ks = [[0u8; 32]; 256];
        ks[self.keys.key_id as usize] = self.keys.aead_key;
        ks
    }
}
