use anyhow::{Context, Result};
use rand::RngCore;
use url::Url;

use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use eframe::egui::{self, TextEdit};

use moq_net::{Origin};

// Import your app logic from the library crate.
use moq_secure_chat::{ChatKeys, ChatPublisher, ChatSubscriber};

// ===================== Types used by the app =====================

#[derive(Clone, Debug)]
pub struct SubscriptionParams {
    pub track: String,
    // publisher identity for decryption
    pub publisher_key_id: u8,
    pub publisher_aead_key: String, // hex or base64
    pub publisher_signing_public_key: String, // hex or base64
}

pub enum EngineCmd {
    SetRelay { relay: Url },
    SetBroadcast { broadcast: String },

    SetPublishTrack { track: String },

    // Create publisher keys (generates if missing) OR overwrite from user-provided inputs.
    SetPublishKeys {
        key_id: u8,
        aead_key: String, // hex or base64
        signing_private_seed_or_bytes: String, // hex or base64,
    },

    Publish { text: String },

    AddSubscription { params: SubscriptionParams },

    RemoveSubscription { track: String },

    Shutdown { reply: oneshot::Sender<()> },
}

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

// ===================== Local UI type =====================

#[derive(Clone, Debug)]
pub struct IncomingMessage {
    pub track: String,
    pub ts: String,
    pub plaintext: String,
}

// ===================== Utilities =====================

fn time_only_hhmmss() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs() % 86_400;
    let hh = secs / 3600;
    let mm = (secs % 3600) / 60;
    let ss = secs % 60;
    format!("{:02}:{:02}:{:02}", hh, mm, ss)
}

fn gen_u8_random() -> u8 {
    let mut b = [0u8; 1];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b[0]
}

fn gen_hex_or_b64_aead_32_bytes_hex() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

fn gen_hex_signing_private_seed_32() -> String {
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    hex::encode(seed)
}

fn ensure_tracking_name(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        "default".to_string()
    } else {
        t.to_string()
    }
}

// ===================== Engine implementation =====================

pub struct ChatEngine {
    cmd_rx: mpsc::Receiver<EngineCmd>,
    incoming_tx: mpsc::Sender<IncomingMessage>,

    // config
    relay_url: Option<Url>,
    broadcast_name: String,

    // publish
    publish_track: String,
    publisher_keys: Option<ChatKeys>,
    publisher: Option<ChatPublisher>,

    // moq client
    moq_client: Option<moq_native::Client>,
    origin: Option<Origin>,

    // active subscribers (one per track)
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

        // Your original code had an API mismatch placeholder; keep it as-is.
        let _ = (client, origin, keys, relay_url);

        return Err(anyhow::anyhow!(
            "moq-net API mismatch: moq_net::broadcast::Broadcast::new not available in this version. Update moq-net usage."
        ));
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

        // ---- IMPORTANT FIX ----
        // The spawned task must own everything it uses (no references to `self`).
        // We move the data we need into the async task.
        let incoming_tx = self.incoming_tx.clone();
        let broadcast_name = self.broadcast_name.clone();

        // If moq_native::Client is Clone, you'd do client.clone() here instead.
        // In case it's NOT Clone (common), we drop the long-lived-client approach:
        // create a fresh client inside the task.
        // (This avoids the 'borrowed data escapes' error.)
        let params_for_task = params.clone();
        let track_for_task = track_key.clone();

        let handle = tokio::spawn(async move {
            // Create a dedicated client + connection for this subscriber task.
            let mut client_cfg = moq_native::ClientConfig::default();
            client_cfg.connect = Some(relay_url);

            let client = client_cfg.init().context("failed to init moq_native client")?;

            let sub_keys = ChatKeys::from_strings_public_verify(
                params_for_task.publisher_key_id,
                &params_for_task.publisher_aead_key,
                &params_for_task.publisher_signing_public_key,
            )
            .context("failed to construct ChatKeys from subscription params")?;

            // NOTE: Your ChatSubscriber / moq-native receive loop depends on moq-net/moq-native APIs
            // in your repo. Your prior code stopped at an API mismatch.
            //
            // Keep the mismatch error for now, but avoid borrowing `self`:
            let _ = (
                incoming_tx,
                broadcast_name,
                origin,
                client,
                sub_keys,
                params_for_task,
                track_for_task
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
                    self.publish_track = ensure_tracking_name(&track);
                    if self.publisher_keys.is_some() && self.relay_url.is_some() {
                        let _ = self.build_publisher_for_current_track().await;
                    }
                }

                EngineCmd::SetPublishKeys {
                    key_id,
                    aead_key,
                    signing_private_seed_or_bytes,
                } => {
                    let keys = ChatKeys::from_strings(
                        key_id,
                        &aead_key,
                        &signing_private_seed_or_bytes,
                    )
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

// ===================== egui app =====================

pub struct ChatApp {
    engine: ChatEngineHandle,
    incoming_rx: mpsc::Receiver<IncomingMessage>,

    relay_url_str: String,
    broadcast_name: String,
    publish_track: String,

    key_id: u8,
    aead_key: String,
    signing_private: String,

    derived_signing_public_key: String,

    sub_track: String,
    sub_publisher_key_id: u8,
    sub_publisher_aead_key: String,
    sub_publisher_signing_public_key: String,

    subscriptions: Vec<SubscriptionParams>,
    messages: HashMap<String, Vec<IncomingMessage>>,

    ui_error: Option<String>,
}

impl ChatApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<EngineCmd>(100);
        let (incoming_tx, incoming_rx) = mpsc::channel::<IncomingMessage>(2000);

        let key_id = gen_u8_random();
        let aead_key = gen_hex_or_b64_aead_32_bytes_hex();
        let signing_private = gen_hex_signing_private_seed_32();

        let derived_signing_public_key = ChatKeys::from_strings(key_id, &aead_key, &signing_private)
            .ok()
            .map(|keys| keys.signing_verify_hex())
            .unwrap_or_default();

        let relay_url_str = "moq://127.0.0.1:5000".to_string();
        let broadcast_name = "chat".to_string();
        let publish_track = "main".to_string();

        let (engine_handle, engine_task) = {
            let engine = ChatEngine::new(cmd_rx, incoming_tx);
            (ChatEngineHandle { cmd_tx }, engine)
        };

        // Spawn engine task on a runtime.
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.spawn(engine_task.run());
        Box::leak(Box::new(rt));

        let relay_url = relay_url_str.parse::<Url>().unwrap_or_else(|_| {
            Url::parse("moq://127.0.0.1:5000").expect("fallback URL parses")
        });

        let _ = engine_handle
            .cmd_tx
            .try_send(EngineCmd::SetRelay { relay: relay_url });
        let _ = engine_handle.cmd_tx.try_send(EngineCmd::SetBroadcast {
            broadcast: broadcast_name.clone(),
        });
        let _ = engine_handle.cmd_tx.try_send(EngineCmd::SetPublishTrack {
            track: publish_track.clone(),
        });
        let _ = engine_handle.cmd_tx.try_send(EngineCmd::SetPublishKeys {
            key_id,
            aead_key: aead_key.clone(),
            signing_private_seed_or_bytes: signing_private.clone(),
        });

        Self {
            engine: engine_handle,
            incoming_rx,

            relay_url_str,
            broadcast_name,
            publish_track,

            key_id,
            aead_key,
            signing_private,

            derived_signing_public_key,

            sub_track: String::new(),
            sub_publisher_key_id: gen_u8_random(),
            sub_publisher_aead_key: gen_hex_or_b64_aead_32_bytes_hex(),
            sub_publisher_signing_public_key: String::new(),

            subscriptions: vec![],
            messages: HashMap::new(),

            ui_error: None,
        }
    }

    fn recalc_derived_public(&mut self) {
        self.ui_error = None;

        match ChatKeys::from_strings(self.key_id, &self.aead_key, &self.signing_private) {
            Ok(keys) => self.derived_signing_public_key = keys.signing_verify_hex(),
            Err(e) => self.ui_error = Some(format!("Publish key parse error: {e}")),
        }
    }
}

impl eframe::App for ChatApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.incoming_rx.try_recv() {
            self.messages
                .entry(msg.track.clone())
                .or_default()
                .push(msg);
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("MOQ Secure Chat (egui)");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        ui.ctx().request_repaint();
                        std::process::exit(0);
                    }
                });
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.heading("Connection / Publish");

                    ui.label("Relay URL (moq://host:port):");
                    ui.add(TextEdit::singleline(&mut self.relay_url_str));

                    ui.label("Broadcast name:");
                    ui.add(TextEdit::singleline(&mut self.broadcast_name));

                    ui.label("Publish track:");
                    ui.add(TextEdit::singleline(&mut self.publish_track));

                    ui.separator();

                    ui.heading("Your publish keys");
                    ui.label(format!("key_id: {}", self.key_id));

                    ui.horizontal(|ui| {
                        if ui.button("Regenerate keys").clicked() {
                            self.key_id = gen_u8_random();
                            self.aead_key = gen_hex_or_b64_aead_32_bytes_hex();
                            self.signing_private = gen_hex_signing_private_seed_32();
                            self.recalc_derived_public();
                        }

                        if ui.button("Send keys to engine").clicked() {
                            self.ui_error = None;
                            match self.relay_url_str.parse::<Url>() {
                                Ok(relay) => {
                                    let _ = self.engine.cmd_tx.try_send(EngineCmd::SetRelay { relay });
                                    let _ = self.engine.cmd_tx.try_send(EngineCmd::SetBroadcast {
                                        broadcast: self.broadcast_name.clone(),
                                    });
                                    let _ = self.engine.cmd_tx.try_send(EngineCmd::SetPublishTrack {
                                        track: self.publish_track.clone(),
                                    });
                                    let _ = self.engine.cmd_tx.try_send(EngineCmd::SetPublishKeys {
                                        key_id: self.key_id,
                                        aead_key: self.aead_key.clone(),
                                        signing_private_seed_or_bytes: self.signing_private.clone(),
                                    });
                                }
                                Err(_) => self.ui_error = Some("Invalid relay URL".to_string()),
                            }
                        }
                    });

                    ui.label("AEAD key (hex or base64, 32 bytes):");
                    ui.add(TextEdit::multiline(&mut self.aead_key).desired_rows(2));

                    ui.label("Signing private seed (hex or base64, 32 bytes) OR 64-byte private key bytes:");
                    ui.add(TextEdit::multiline(&mut self.signing_private).desired_rows(2));

                    ui.label("Your signing public verify key (hex shown; copy it):");
                    ui.add(
                        TextEdit::multiline(&mut self.derived_signing_public_key)
                            .desired_rows(2),
                    );

                    if let Some(e) = &self.ui_error {
                        ui.colored_label(egui::Color32::RED, e);
                    }

                    ui.separator();

                    ui.heading("Publish message");
                    let mut input = String::new();
                    ui.label("Message:");
                    ui.add(TextEdit::multiline(&mut input).desired_rows(3));

                    if ui.button("Publish to current track").clicked() {
                        let text = input.trim().to_string();
                        if !text.is_empty() {
                            let _ = self.engine.cmd_tx.try_send(EngineCmd::Publish { text });
                        }
                    }
                });

                ui.separator();

                ui.vertical(|ui| {
                    ui.heading("Subscriptions (multi-track)");

                    ui.label("Track name to subscribe:");
                    ui.add(TextEdit::singleline(&mut self.sub_track));

                    ui.label("Publisher key_id (u8):");
                    ui.add(egui::DragValue::new(&mut self.sub_publisher_key_id));

                    ui.label("Publisher AEAD key (hex or base64, 32 bytes):");
                    ui.add(
                        TextEdit::multiline(&mut self.sub_publisher_aead_key).desired_rows(2),
                    );

                    ui.label("Publisher signing public verify key (hex or base64):");
                    ui.add(
                        TextEdit::multiline(&mut self.sub_publisher_signing_public_key)
                            .desired_rows(2),
                    );

                    ui.horizontal(|ui| {
                        if ui.button("Add subscription").clicked() {
                            let track = ensure_tracking_name(&self.sub_track);

                            if !self.sub_publisher_signing_public_key.trim().is_empty()
                                && !self.sub_publisher_aead_key.trim().is_empty()
                            {
                                let params = SubscriptionParams {
                                    track: track.clone(),
                                    publisher_key_id: self.sub_publisher_key_id,
                                    publisher_aead_key: self.sub_publisher_aead_key.clone(),
                                    publisher_signing_public_key: self
                                        .sub_publisher_signing_public_key
                                        .clone(),
                                };

                                self.subscriptions.retain(|s| s.track != track);
                                self.subscriptions.push(params.clone());

                                let _ = self
                                    .engine
                                    .cmd_tx
                                    .try_send(EngineCmd::AddSubscription { params });
                            }
                        }

                        if ui.button("Remove subscription (by track)").clicked() {
                            let track = self.sub_track.trim().to_string();
                            if !track.is_empty() {
                                self.subscriptions.retain(|s| s.track != track);
                                let _ =
                                    self.engine.cmd_tx.try_send(EngineCmd::RemoveSubscription { track });
                            }
                        }
                    });

                    ui.separator();

                    ui.heading("Message feed");
                    egui::ScrollArea::vertical()
                        .max_height(600.0)
                        .show(ui, |ui| {
                            for sub in &self.subscriptions {
                                let msgs = self.messages.get(&sub.track);

                                ui.group(|ui| {
                                    ui.heading(format!("Track: {}", sub.track));
                                    if let Some(list) = msgs {
                                        for m in list.iter().rev().take(200) {
                                            ui.label(format!("[{}] {}", m.ts, m.plaintext));
                                        }
                                    } else {
                                        ui.label("No messages yet.");
                                    }
                                });
                                ui.separator();
                            }
                        });
                });
            });
        });
    }
}

// ===================== eframe main =====================

pub fn run_gui() -> Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "MOQ Secure Chat",
        options,
        Box::new(|cc| Ok(Box::new(ChatApp::new(cc)))),
    )?;
    Ok(())
}

fn main() -> Result<()> {
    run_gui()
}
