use anyhow::{Context, Result};
use rand::RngCore;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use url::Url;

use moq_net::Origin;
use moq_secure_chat::{ChatKeys, ChatPublisher};

use crate::types::{IncomingMessage, SubscriptionParams};

#[derive(Clone, Debug)]
pub struct ChatEngineHandle {
    pub cmd_tx: mpsc::Sender<EngineCmd>,
}

impl ChatEngineHandle {
    pub async fn shutdown(&self) {
        let (tx, rx) = oneshot::channel();
        let _ = self.cmd_tx.send(EngineCmd::Shutdown { reply: tx }).await;
        let _ = rx.await;
    }
}

pub enum EngineCmd {
    SetRelay { relay: Url },
    SetBroadcast { broadcast: String },

    SetPublishTrack { track: String },

    SetPublishKeys {
        key_id: u8,
        aead_key: String,
        signing_private_seed_or_bytes: String,
    },

    Publish { text: String },

    AddSubscription { params: SubscriptionParams },
    RemoveSubscription { track: String },

    Shutdown { reply: oneshot::Sender<()> },
}

pub struct ChatEngine {
    cmd_rx: mpsc::Receiver<EngineCmd>,
    incoming_tx: mpsc::Sender<IncomingMessage>,

    relay_url: Option<Url>,
    broadcast_name: String,

    publish_track: String,
    publisher_keys: Option<ChatKeys>,
    publisher: Option<ChatPublisher>,

    moq_client: Option<moq_native::Client>,
    origin: Option<Origin>,

    subs: HashMap<String, JoinHandle<Result<()>>>,
}

impl ChatEngine {
    pub fn new(
        cmd_rx: mpsc::Receiver<EngineCmd>,
        incoming_tx: mpsc::Sender<IncomingMessage>,
    ) -> Self {
        Self {
            cmd_rx,
            incoming_tx,
            relay_url: None,
            broadcast_name: "broadcast".to_string(),
            publish_track: "track".to_string(),
            publisher_keys: None,
            publisher: None,
            moq_client: None,
            origin: None,
            subs: HashMap::new(),
        }
    }

    fn init_client_if_needed(&mut self, relay_url: &Url) -> Result<()> {
        if self.moq_client.is_some() {
            return Ok(());
        }

        let mut client_cfg = moq_native::ClientConfig::default();
        client_cfg.connect = Some(relay_url.clone());

        let client = client_cfg.init()?;
        self.moq_client = Some(client);

        let origin = Origin::random();
        self.origin = Some(origin);

        Ok(())
    }

    async fn build_publisher_for_current_track(&mut self) -> Result<()> {
        let relay_url = self
            .relay_url
            .clone()
            .context("relay_url not set yet")?;
        self.init_client_if_needed(&relay_url)?;

        let origin = self.origin.clone().context("origin missing")?;
        let client = self.moq_client.as_mut().context("client missing")?;

        let keys = self.publisher_keys.clone().context("publisher keys not set")?;

        let _ = (client, origin, keys, relay_url);

        Err(anyhow::anyhow!(
            "moq-net API mismatch: moq_net::broadcast::Broadcast::new not available in this version. Update moq-net usage."
        ))
    }

    async fn spawn_subscriber_task(&mut self, params: SubscriptionParams) -> Result<()> {
        let relay_url = self
            .relay_url
            .clone()
            .context("relay_url not set yet")?;

        self.init_client_if_needed(&relay_url)?;

        let origin = self.origin.clone().context("origin missing")?;

        let track_key = params.track.clone();
        if self.subs.contains_key(&track_key) {
            return Ok(());
        }

        let incoming_tx = self.incoming_tx.clone();
        let broadcast_name = self.broadcast_name.clone();

        let params_for_task = params.clone();
        let track_for_task = track_key.clone();

        let handle = tokio::spawn(async move {
            let mut client_cfg = moq_native::ClientConfig::default();
            client_cfg.connect = Some(relay_url);

            let client = client_cfg
                .init()
                .context("failed to init moq_native client")?;

            let sub_keys = ChatKeys::from_strings_public_verify(
                params_for_task.publisher_key_id,
                &params_for_task.publisher_aead_key,
                &params_for_task.publisher_signing_public_key,
            )
            .context("failed to construct ChatKeys from subscription params")?;

            let _ = (
                incoming_tx,
                broadcast_name,
                origin,
                client,
                sub_keys,
                params_for_task,
                track_for_task,
            );

            Err(anyhow::anyhow!(
                "moq-native API mismatch: subscriber receive loop must be updated for this moq-native version (e.g., replace next_update with the correct update/recv API)."
            ))
        });

        self.subs.insert(track_key, handle);
        Ok(())
    }

    pub async fn run(mut self) -> Result<()> {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                EngineCmd::Shutdown { reply } => {
                    for (_, h) in self.subs.drain() {
                        h.abort();
                    }
                    let _ = reply.send(());
                    break;
                }

                EngineCmd::SetRelay { relay } => {
                    self.relay_url = Some(relay);
                }

                EngineCmd::SetBroadcast { broadcast } => {
                    self.broadcast_name = broadcast;
                }

                EngineCmd::SetPublishTrack { track } => {
                    self.publish_track = crate::util::ensure_tracking_name(&track);
                    if self.publisher_keys.is_some() && self.relay_url.is_some() {
                        let _ = self.build_publisher_for_current_track().await;
                    }
                }

                EngineCmd::SetPublishKeys {
                    key_id,
                    aead_key,
                    signing_private_seed_or_bytes,
                } => {
                    let keys = ChatKeys::from_strings(key_id, &aead_key, &signing_private_seed_or_bytes)
                        .context("failed to create publisher ChatKeys from UI-provided keys")?;
                    self.publisher_keys = Some(keys);

                    if self.relay_url.is_some() {
                        let _ = self.build_publisher_for_current_track().await;
                    }
                }

                EngineCmd::Publish { text } => {
                    if let Some(pubr) = self.publisher.as_mut() {
                        let _ = pubr.send_message(text.as_bytes()).await;
                    }
                }

                EngineCmd::AddSubscription { params } => {
                    let _ = self.spawn_subscriber_task(params).await;
                }

                EngineCmd::RemoveSubscription { track } => {
                    if let Some(h) = self.subs.remove(&track) {
                        h.abort();
                    }
                }
            }
        }

        Ok(())
    }
}
