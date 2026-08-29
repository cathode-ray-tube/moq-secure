use anyhow::Context;
use ed25519_dalek::VerifyingKey;
use moq_secure::{
    wire::{decrypt_frame, encrypt_frame},
    InMemoryKeyStore,
};

use crate::keys::{PublisherKeys, SubscriberKeys};

pub struct ChatSession {
    pub broadcast: String,
    pub track: String,
}

impl ChatSession {
    pub fn new(broadcast: String, track: String) -> Self {
        Self { broadcast, track }
    }
}

/// Publisher publishes each chat message as one MoQ group containing one
/// frame.
pub struct ChatPublisher {
    pub keys: PublisherKeys,
    pub track: moq_net::track::Producer,
    pub n_signed: u8,
    pub crypto_ctr: u64,
    pub group_ctr: u64,
}

impl ChatPublisher {
    pub fn new(
        track: moq_net::track::Producer,
        keys: PublisherKeys,
    ) -> Self {
        Self {
            keys,
            track,
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

    pub async fn send_message(
        &mut self,
        plaintext: &[u8],
    ) -> anyhow::Result<()> {
        let crypto_ctr = self.crypto_ctr;
        self.crypto_ctr = self.crypto_ctr.wrapping_add(1);

        let group_id = self.group_ctr;
        self.group_ctr = self.group_ctr.wrapping_add(1);

        let keystore = self.keystore();

        let frame = encrypt_frame(
            &keystore,
            &self.keys.signing_key,
            self.keys.key_id,
            crypto_ctr,
            self.n_signed,
            true,
            1,
            0,
            plaintext,
        )
        .context("encrypt_frame failed")?;

        let mut group = self
            .track
            .create_group(group_id.into())
            .context("create_group failed")?;

        group.write_frame(
            moq_native::moq_net::Timestamp::now(),
            frame.serialize(),
        )?;

        group.finish()?;
        Ok(())
    }

    pub fn finish(self) {
        drop(self.track);
    }
}

/// Subscriber receives chat frames, verifies their signatures, and decrypts
/// them. It has no private signing key.
pub struct ChatSubscriber {
    pub verify_key: VerifyingKey,
    pub keys: SubscriberKeys,
    pub track: moq_net::track::Subscriber,
    pub lease_remaining: u8,
}

impl ChatSubscriber {
    pub fn new(
        track: moq_net::track::Subscriber,
        keys: SubscriberKeys,
    ) -> Self {
        let verify_key = keys.signing_verify_key;

        Self {
            verify_key,
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
