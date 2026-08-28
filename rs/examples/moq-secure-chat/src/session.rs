use anyhow::Context;
use moq_secure::{
    wire::{decrypt_frame, encrypt_frame},
    InMemoryKeyStore,
};

use crate::keys::ChatKeys;

pub struct ChatSession {
    pub keys: ChatKeys,
    pub broadcast: String,
    pub track: String,
}

impl ChatSession {
    pub fn new(keys: ChatKeys, broadcast: String, track: String) -> Self {
        Self {
            keys,
            broadcast,
            track,
        }
    }

    fn keystore(&self) -> InMemoryKeyStore {
        let mut keystore = InMemoryKeyStore::empty();
        keystore.set_key(self.keys.key_id, self.keys.aead_key);
        keystore
    }
}

/// Publisher publishes each chat message as one MoQ group containing one frame.
///
/// One chat message equals one group containing one object.
pub struct ChatPublisher {
    pub keys: ChatKeys,
    pub track: moq_net::track::Producer,
    pub n_signed: u8,
    pub crypto_ctr: u64,
    pub group_ctr: u64,
}

impl ChatPublisher {
    pub fn new(track: moq_net::track::Producer, keys: ChatKeys) -> Self {
        Self {
            keys,
            track,
            // Every message is encrypted and signed.
            n_signed: 1,
            crypto_ctr: 0,
            group_ctr: 0,
        }
    }

    fn keystore(&self) -> InMemoryKeyStore {
        let mut keystore = InMemoryKeyStore::empty();
        keystore.set_key(self.keys.key_id, self.keys.aead_key);
        keystore
    }

    pub async fn send_message(&mut self, plaintext: &[u8]) -> anyhow::Result<()> {
        let crypto_ctr = self.crypto_ctr;
        self.crypto_ctr = self.crypto_ctr.wrapping_add(1);

        let group_id = self.group_ctr;
        self.group_ctr = self.group_ctr.wrapping_add(1);

        // No padding for now.
        let pad_len = 0u32;

        // Build the keystore expected by moq-secure.
        let keystore = self.keystore();

        let frame = encrypt_frame(
            &keystore,
            &self.keys.signing_private,
            self.keys.key_id,
            crypto_ctr,
            self.n_signed,
            true,
            1,
            pad_len,
            plaintext,
        )
        .context("encrypt_frame failed")?;

        let frame_bytes = frame.serialize();

        // group_id must be unique across sends.
        let mut group = self
            .track
            .create_group(group_id.into())
            .context("create_group")?;

        group.write_frame(
            moq_native::moq_net::Timestamp::now(),
            frame_bytes,
        )?;

        group.finish()?;

        Ok(())
    }

    pub fn finish(self) {
        // Dropping the producer normally closes it.
        drop(self.track);
    }
}

/// Subscriber receives chat frames, verifies their signatures, and decrypts them.
pub struct ChatSubscriber {
    pub verify_key: ed25519_dalek::VerifyingKey,
    pub keys: ChatKeys,
    pub track: moq_net::track::Subscriber,
    pub lease_remaining: u8,
}

impl ChatSubscriber {
    pub fn new(
        track: moq_net::track::Subscriber,
        keys: ChatKeys,
    ) -> Self {
        Self {
            verify_key: keys.signing_verify,
            keys,
            track,
            lease_remaining: 0,
        }
    }

    fn keystore(&self) -> InMemoryKeyStore {
        let mut keystore = InMemoryKeyStore::empty();
        keystore.set_key(self.keys.key_id, self.keys.aead_key);
        keystore
    }

    pub async fn run(
        mut self,
        mut on_message: impl FnMut(Vec<u8>) + Send,
    ) -> anyhow::Result<()> {
        let keystore = self.keystore();

        while let Some(mut group) = self.track.recv_group().await? {
            while let Some(object) = group.read_frame().await? {
                let plaintext = decrypt_frame(
                    &keystore,
                    &self.verify_key,
                    &mut self.lease_remaining,
                    &object.payload,
                )
                .context("decrypt_frame failed")?;

                on_message(plaintext);
            }
        }

        Ok(())
    }
}
