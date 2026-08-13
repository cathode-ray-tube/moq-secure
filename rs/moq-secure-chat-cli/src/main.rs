use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use chrono::Local;
use moq_net::{Origin, Path};
use moq_secure_chat::{ChatKeys, ChatPublisher, ChatSubscriber};
use rand::RngCore;
use url::Url;

use std::env;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
struct Cli {
    /// MoQ relay endpoint (same format as moq-native examples)
    #[arg(long)]
    relay: String,

    /// Broadcast name
    #[arg(long)]
    broadcast: String,

    /// Track name. Required in subscribe mode. Generated in publish mode if not supplied.
    #[arg(long)]
    track: Option<String>,

    /// Disable TLS cert verification (for localhost testing; forwarded to moq-native)
    #[arg(long, default_value_t = false)]
    tls_disable_verify: bool,

    #[command(subcommand)]
    role: Role,

    /// Optional overrides for keys (accept hex or base64 for aead key)
    #[arg(long)]
    key_id: Option<u8>,

    #[arg(long)]
    aead_key: Option<String>,

    /// Ed25519 private key seed (preferred) as hex or base64.
    /// Optional: if omitted, publish will generate one and print copy/paste subscribe args.
    #[arg(long)]
    signing_private_seed: Option<String>,

    /// Ed25519 signing public verify key (32 bytes) as hex or base64.
    /// Required in subscribe mode.
    #[arg(long)]
    signing_public_key: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
enum Role {
    /// Publish messages to the track. Reads stdin until Ctrl+C; each line is one chat message.
    Publish {},
    /// Subscribe and print decrypted messages.
    Subscribe,
}

fn random_track_hex() -> String {
    let mut b = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn gen_key_id() -> u8 {
    rand::random()
}

fn gen_aead_key_hex() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

fn gen_signing_private_seed_hex() -> String {
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

    let track = match (&cli.role, &cli.track) {
        (Role::Publish {}, Some(t)) => t.clone(),
        (Role::Publish {}, None) => random_track_hex(),
        (Role::Subscribe, Some(t)) => t.clone(),
        (Role::Subscribe, None) => anyhow::bail!("--track is required for subscribe mode"),
    };

    let key_id: u8 = cli.key_id.unwrap_or_else(gen_key_id);
    let aead_key: String = cli.aead_key.unwrap_or_else(gen_aead_key_hex);

    let relay_url: Url = cli
        .relay
        .parse()
        .context("invalid --relay url (expected a URL like moq://host:port or similar)")?;

    let mut client_cfg = moq_native::ClientConfig::default();
    client_cfg.connect = Some(relay_url.clone());
    client_cfg.tls.disable_verify = Some(cli.tls_disable_verify);
    let client = client_cfg.init()?;

    let origin = Origin::random().produce();

    match cli.role {
         Role::Publish {} => {
            let signing_private_seed_or_bytes =
                cli.signing_private_seed.unwrap_or_else(gen_signing_private_seed_hex);

            let keys =
                ChatKeys::from_strings(key_id, &aead_key, &signing_private_seed_or_bytes)
                    .context("failed to construct ChatKeys from provided/generated values")?;

            let mut broadcast = origin
                .create_broadcast(
                    &cli.broadcast,
                    moq_net::broadcast::Route::new().with_announce(true),
                )
                .context("failed to create broadcast")?;

            let track_producer = broadcast
                .create_track(track.clone(), None)
                .context("failed to create track")?;

            let pwd_bin: PathBuf = env::current_dir()?.join("moq-secure-chat-cli");
            let pwd_bin_str = pwd_bin.to_string_lossy();
            let pwd_bin_escaped = shell_escape(&pwd_bin_str);

            let subscribe_cmd = if cli.tls_disable_verify {
                format!(
                    "{bin} --relay {} --broadcast {} --track {} --key-id {} --aead-key {} --signing-public-key {} --tls-disable-verify subscribe",
                    shell_escape(&cli.relay),
                    shell_escape(&cli.broadcast),
                    shell_escape(&track),
                    keys.key_id,
                    shell_escape(&keys.aead_key_hex()),
                    shell_escape(&keys.signing_verify_hex()),
                    bin = pwd_bin_escaped
                )
            } else {
                format!(
                    "{bin} --relay {} --broadcast {} --track {} --key-id {} --aead-key {} --signing-public-key {} subscribe",
                    shell_escape(&cli.relay),
                    shell_escape(&cli.broadcast),
                    shell_escape(&track),
                    keys.key_id,
                    shell_escape(&keys.aead_key_hex()),
                    shell_escape(&keys.signing_verify_hex()),
                    bin = pwd_bin_escaped
                )
            };

            println!("=== Copy/paste subscriber command (run in another terminal) ===");
            println!("{subscribe_cmd}");
            println!("=== Publisher running ===");
            println!("Type lines on stdin; each line is one chat message. Press Ctrl+C to quit.\n");

            let publisher = ChatPublisher::new(track_producer, keys);
            let reconnect = client.with_publisher(&origin).reconnect(relay_url);

            let mut publisher = publisher;

            let mut ctrl_c_fut = tokio::signal::ctrl_c();
            tokio::pin!(ctrl_c_fut);

            let mut stdin_task = tokio::spawn(async move {
                use tokio::io::{self, AsyncBufReadExt};

                let stdin = io::stdin();
                let mut reader = io::BufReader::new(stdin).lines();

                // Separate Ctrl+C inside the stdin task so it can stop immediately
                let mut ctrl_c_fut2 = tokio::signal::ctrl_c();
                tokio::pin!(ctrl_c_fut2);

                loop {
                    tokio::select! {
                        _ = &mut ctrl_c_fut2 => {
                            return Ok::<(), anyhow::Error>(());
                        }
                        line = reader.next_line() => {
                            let Some(text) = line? else { return Ok::<(), anyhow::Error>(()); };
                            publisher.send_message(text.as_bytes()).await?;
                        }
                    }
                }
            });

            let result: Result<()> = tokio::select! {
                res = reconnect.closed() => res.map_err(Into::into),

                res = &mut stdin_task => {
                    let inner: Result<()> = res.map_err(|e| anyhow::anyhow!(e))?;
                    inner
                },

                _ = &mut ctrl_c_fut => {
                    stdin_task.abort();
                    Ok(())
                }
            };

            broadcast.finish();
            result
        }

        Role::Subscribe => {
            let signing_public = cli
                .signing_public_key
                .context("--signing-public-key is required for subscribe mode")?;

            let keys = ChatKeys::from_strings_public_verify(key_id, &aead_key, &signing_public)
                .context("failed to construct ChatKeys (public-verify)")?;

            let reconnect = client.with_subscriber(origin.clone()).reconnect(relay_url);

            let path: Path<'_> = cli.broadcast.as_str().into();

            let mut origin = origin
                .scope(&[path])
                .context("not allowed to consume broadcast")?
                .consume()
                .announced();

            tracing::info!(
                broadcast = %cli.broadcast,
                track = %track,
                "waiting for broadcast to be online"
            );

            let mut subscriber_task: Option<tokio::task::JoinHandle<Result<()>>> = None;

            loop {
                if let Some(handle) = subscriber_task.take() {
                    tokio::select! {
                        res = reconnect.closed() => return Ok(res?),

                        res = handle => {
                            res.map_err(|e| anyhow::anyhow!(e))??;
                            return Ok(());
                        }

                        Some(moq_net::announce::Update { broadcast, .. }) = origin.next() => {
                            match broadcast {
                                Some(b) => {
                                    tracing::info!("broadcast is online, subscribing to track");

                                    let track_sub = b.track(&track)?.subscribe(None).await?;

                                    let track_name = track.clone();
                                    let keys_clone = keys.clone();

                                    subscriber_task = Some(tokio::spawn(async move {
                                        let subscriber = ChatSubscriber::new(track_sub, keys_clone);
                                        subscriber
                                            .run(move |pt| {
                                                if let Ok(s) = String::from_utf8(pt.clone()) {
                                                    let ts = time_only_hhmmss();
                                                    println!("[{}] {}: {}", ts, track_name, s);
                                                } else {
                                                    eprintln!("(non-utf8 message) {} bytes", pt.len());
                                                }
                                            })
                                            .await?;
                                        Ok::<(), anyhow::Error>(())
                                    }));
                                }
                                None => {
                                    tracing::warn!("broadcast offline, waiting...");
                                    subscriber_task = None;
                                }
                            }
                        }
                    }
                } else {
                    tokio::select! {
                        res = reconnect.closed() => return Ok(res?),

                        Some(moq_net::announce::Update { broadcast, .. }) = origin.next() => {
                            match broadcast {
                                Some(b) => {
                                    tracing::info!("broadcast is online, subscribing to track");

                                    let track_sub = b.track(&track)?.subscribe(None).await?;

                                    let track_name = track.clone();
                                    let keys_clone = keys.clone();

                                    subscriber_task = Some(tokio::spawn(async move {
                                        let subscriber = ChatSubscriber::new(track_sub, keys_clone);
                                        subscriber
                                            .run(move |pt| {
                                                if let Ok(s) = String::from_utf8(pt.clone()) {
                                                    let ts = time_only_hhmmss();
                                                    println!("[{}] {}: {}", ts, track_name, s);
                                                } else {
                                                    eprintln!("(non-utf8 message) {} bytes", pt.len());
                                                }
                                            })
                                            .await?;
                                        Ok::<(), anyhow::Error>(())
                                    }));
                                }
                                None => {
                                    tracing::warn!("broadcast offline, waiting...");
                                    subscriber_task = None;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
