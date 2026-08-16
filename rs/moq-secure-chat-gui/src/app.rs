use std::collections::HashMap;

use eframe::egui::{self, TextEdit};
use moq_secure_chat::ChatKeys;
use url::Url;

use tokio::sync::mpsc;

use crate::engine::{ChatEngine, ChatEngineHandle, EngineCmd};
use crate::types::{IncomingMessage, SubscriptionParams};
use crate::util::{
    gen_hex_or_b64_aead_32_bytes_hex, gen_hex_signing_private_seed_32, gen_u8_random,
    ensure_tracking_name,
};

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

        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.spawn(engine_task.run());
        Box::leak(Box::new(rt));

        let relay_url = relay_url_str.parse::<Url>().unwrap_or_else(|_| {
            Url::parse("moq://127.0.0.1:5000").expect("fallback URL parses")
        });

        let _ = engine_handle.cmd_tx.try_send(EngineCmd::SetRelay { relay: relay_url });
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
            self.messages.entry(msg.track.clone()).or_default().push(msg);
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("MOQ Secure Chat (egui)");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
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
                    ui.add(TextEdit::multiline(&mut self.sub_publisher_aead_key).desired_rows(2));

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
                                    publisher_signing_public_key: self.sub_publisher_signing_public_key.clone(),
                                };

                                self.subscriptions.retain(|s| s.track != track);
                                self.subscriptions.push(params.clone());

                                let _ = self.engine.cmd_tx.try_send(EngineCmd::AddSubscription { params });
                            }
                        }

                        if ui.button("Remove subscription (by track)").clicked() {
                            let track = self.sub_track.trim().to_string();
                            if !track.is_empty() {
                                self.subscriptions.retain(|s| s.track != track);
                                let _ = self.engine.cmd_tx.try_send(EngineCmd::RemoveSubscription { track });
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
