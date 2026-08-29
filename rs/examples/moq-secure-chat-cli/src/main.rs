use anyhow::{Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand};
use moq_net::{Origin, Path};
use moq_secure_chat::{ChatPublisher, ChatSubscriber, PublisherKeys, SubscriberKeys};
use rand::RngCore;
use url::Url;

use std::env;
use std::io::{self, BufRead};
use std::path::PathBuf;
use tokio::sync::mpsc;

const CHAT_TRACK: &str = "chat";

#[derive(Parser, Debug, Clone)]
struct Cli {
    /// MoQ relay endpoint
    #[arg(long)]
    relay: String,

    /// Broadcast name. Generated in publish mode if omitted.
    #[arg(long)]
    broadcast: Option<String>,

    /// Disable TLS cert verification
    #[arg(long, default_value_t = false)]
    tls_disable_verify: bool,

    #[command(subcommand)]
    role: Role,

    /// Optional override for the key ID
    #[arg(long)]
    key_id: Option<u8>,

    /// Optional override for the AEAD key
    #[arg(long)]
    aead_key: Option<String>,

    /// Ed25519 signing seed: exactly 32 raw bytes encoded as hex or Base64.
    ///
    /// This is the seed used to derive the private signing key. It is not a
    /// PEM, PKCS#8, OpenSSH, expanded, or 64-byte private-key representation.
    #[arg(long = "ed25519-signing-seed", alias = "signing-private-seed")]
    ed25519_signing_seed: Option<String>,

    /// Ed25519 signing public verification key as hex or Base64.
    /// Required in subscribe mode.
    #[arg(long)]
    signing_public_key: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
enum Role {
    /// Publish messages to this participant's broadcast.
    Publish {},

    /// Subscribe to a participant broadcast.
    Subscribe,
}

fn random_broadcast_hex() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn gen_key_id() -> u8 {
    rand::random()
}

fn gen_aead_key_hex() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn gen_signing_seed_hex() -> String {
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    hex::encode(seed)
}

fn print_launch_art() {
    println!("  __  __           ____                  ");
    println!(" |  \\/  |         / __ \\                 ");
    println!(" | \\  / |  ___  | |  | |_   _  ___ _ __");
    println!(" | |\\/| | / _ \\ | |  | | | | |/ _ \\ '__|");
    println!(" | |  | || (_) || |__| | |_| |  __/ |   ");
    println!(" |_|  |_| \\___/  \\____/ \\__,_|\\___|_|   ");
    println!("               MOQ-Secure\n");
}

fn time_only_hhmmss() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    print_launch_art();

    let cli = Cli::parse();

    let broadcast_name = match (&cli.role, &cli.broadcast) {
        (Role::Publish {}, Some(name)) => name.clone(),
        (Role::Publish {}, None) => random_broadcast_hex(),
        (Role::Subscribe, Some(name)) => name.clone(),
        (Role::Subscribe, None) => {
            anyhow::bail!("--broadcast is required in subscribe mode");
        }
    };

    let key_id = cli.key_id.unwrap_or_else(gen_key_id);
    let aead_key = cli.aead_key.unwrap_or_else(gen_aead_key_hex);

    let relay_url: Url = cli
        .relay
        .parse()
        .context("invalid --relay URL")?;

    let mut client_cfg = moq_native::ClientConfig::default();
    client_cfg.connect = Some(relay_url.clone());
    client_cfg.tls.disable_verify = Some(cli.tls_disable_verify);

    let client = client_cfg.init()?;
    let origin = Origin::random().produce();

    match cli.role {
        Role::Publish {} => {
            let signing_seed = cli
                .ed25519_signing_seed
                .unwrap_or_else(gen_signing_seed_hex);

            let keys = PublisherKeys::from_strings(
                key_id,
                &aead_key,
                &signing_seed,
            )
            .context(
                "failed to construct publisher keys; \
                 the Ed25519 signing seed must decode to exactly 32 bytes",
            )?;

            let mut broadcast = origin
                .create_broadcast(
                    &broadcast_name,
                    moq_net::broadcast::Route::new().with_announce(true),
                )
                .context("failed to create broadcast")?;

            // Every participant gets a unique broadcast, while all chat
            // messages use the same track name within that broadcast.
            let track_producer = broadcast
                .create_track(CHAT_TRACK.to_owned(), None)
                .context("failed to create chat track")?;

            let pwd_bin: PathBuf =
                env::current_dir()?.join("moq-secure-chat-cli");
            let bin = shell_escape(&pwd_bin.to_string_lossy());

            let tls_flag = if cli.tls_disable_verify {
                " --tls-disable-verify"
            } else {
                ""
            };

            let subscribe_cmd = format!(
                "{bin} --relay {relay} --broadcast {broadcast} \
                 --key-id {key_id} --aead-key {aead_key} \
                 --signing-public-key {signing_key}{tls} subscribe",
                bin = bin,
                relay = shell_escape(&cli.relay),
                broadcast = shell_escape(&broadcast_name),
                key_id = keys.key_id,
                aead_key = shell_escape(&keys.aead_key_hex()),
                signing_key = shell_escape(&keys.signing_verify_hex()),
                tls = tls_flag,
            );

            println!("=== Copy/paste subscriber command ===");
            println!("{subscribe_cmd}");
            println!("=== Publisher running ===");
            println!("Broadcast: {broadcast_name}");
            println!("Track: {CHAT_TRACK}");
            println!("Type lines on stdin; press Ctrl+C to quit.\n");

            let publisher = ChatPublisher::new(track_producer, keys);
            let reconnect = client
                .with_publisher(&origin)
                .reconnect(relay_url);

            let (tx, mut rx) = mpsc::channel::<String>(100);

            std::thread::spawn(move || {
                let stdin = io::stdin();
                let reader = stdin.lock();

                for line in reader.lines() {
                    match line {
                        Ok(text) => {
                            let _ = tx.blocking_send(text);
                        }
                        Err(_) => break,
                    }
                }
            });

            let mut ctrl_c = tokio::signal::ctrl_c();
            tokio::pin!(ctrl_c);

            let stdin_task = tokio::spawn(async move {
                let mut publisher = publisher;

                while let Some(text) = rx.recv().await {
                    publisher
                        .send_message(text.as_bytes())
                        .await
                        .context("failed to send chat message")?;
                }

                Ok::<(), anyhow::Error>(())
            });

            let result: Result<()> = tokio::select! {
                res = reconnect.closed() => {
                    res.map_err(Into::into)
                }

                res = stdin_task => {
                    res.map_err(|error| anyhow::anyhow!(error))??;
                    Ok(())
                }

                _ = &mut ctrl_c => Ok(()),
            };

            broadcast.finish();
            result
        }

        Role::Subscribe => {
            let signing_public = cli
                .signing_public_key
                .context("--signing-public-key is required in subscribe mode")?;

            let keys = SubscriberKeys::from_strings(
                key_id,
                &aead_key,
                &signing_public,
            )
            .context("failed to construct subscriber keys")?;

            let reconnect = client
                .with_subscriber(origin.clone())
                .reconnect(relay_url);

            let path: Path<'_> = broadcast_name.as_str().into();

            let mut origin = origin
                .scope(&[path])
                .context("not allowed to consume broadcast")?
                .consume()
                .announced();

            tracing::info!(
                broadcast = %broadcast_name,
                track = CHAT_TRACK,
                "waiting for broadcast"
            );

            loop {
                tokio::select! {
                    res = reconnect.closed() => {
                        return Ok(res?);
                    }

                    Some(moq_net::announce::Update { broadcast, .. }) = origin.next() => {
                        match broadcast {
                            Some(broadcast) => {
                                tracing::info!(
                                    broadcast = %broadcast_name,
                                    track = CHAT_TRACK,
                                    "broadcast is online"
                                );

                                let track_sub =
                                    broadcast.track(CHAT_TRACK)?.subscribe(None).await?;

                                let keys_clone = keys.clone();

                                tokio::spawn(async move {
                                    let subscriber =
                                        ChatSubscriber::new(track_sub, keys_clone);

                                    subscriber
                                        .run(move |payload| {
                                            match String::from_utf8(payload.clone()) {
                                                Ok(message) => {
                                                    println!(
                                                        "[{}] {}",
                                                        time_only_hhmmss(),
                                                        message
                                                    );
                                                }

                                                Err(_) => {
                                                    eprintln!(
                                                        "(non-UTF-8 message) {} bytes",
                                                        payload.len()
                                                    );
                                                }
                                            }
                                        })
                                        .await
                                });
                            }

                            None => {
                                tracing::warn!(
                                    "broadcast offline, waiting..."
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
