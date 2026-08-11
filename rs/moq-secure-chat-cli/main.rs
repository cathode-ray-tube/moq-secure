use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use moq_net::{Origin, Path};
use moq_secure_chat::{ChatKeys, ChatPublisher, ChatSubscriber};

#[derive(Parser, Debug, Clone)]
struct Cli {
    /// MoQ relay endpoint (same format as moq-native examples)
    #[arg(long)]
    relay: String,

    /// Broadcast name
    #[arg(long)]
    broadcast: String,

    /// Track name. Required in subscribe mode.
    #[arg(long)]
    track: Option<String>,

    /// Disable TLS cert verification (for localhost testing; forwarded to moq-native)
    #[arg(long, default_value_t = false)]
    tls_disable_verify: bool,

    #[command(subcommand)]
    role: Role,

    /// Optional overrides for keys (accept hex or base64)
    #[arg(long)]
    key_id: Option<u8>,

    #[arg(long)]
    aead_key: Option<String>,

    /// Ed25519 private key seed (preferred) as hex or base64; CLI prints this in hex.
    #[arg(long)]
    signing_private_seed: Option<String>,

    /// Subscriber: verify public key as hex or base64 (seed is not needed for verification).
    /// If omitted, CLI will derive verify from signing_private_seed (publisher mode prints seed).
    #[arg(long)]
    verify_public_key: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
enum Role {
    /// Publish messages to the track. CLI prints a copy-pastable subscribe command.
    Publish {
        /// Publish a single message then exit (UTF-8). If omitted, read stdin until EOF.
        #[arg(long)]
        message: Option<String>,
    },
    /// Subscribe and print decrypted messages.
    Subscribe,
}

fn random_track_hex() -> String {
    let mut b = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

fn build_subscribe_command(base: &Cli, track: &str, keys: &ChatKeys) -> String {
    let cmd = format!(
        "moq-secure-chat-cli --relay {} --broadcast {} --track {} --tls-disable-verify{} \
--key-id {} --aead-key {} --signing-private-seed {}",
        shell_escape(&base.relay),
        shell_escape(&base.broadcast),
        shell_escape(track),
        if base.tls_disable_verify { "" } else { "" }, // still include flag only if set; kept simple below
        keys.key_id,
        shell_escape(&keys.aead_key_hex()),
        shell_escape(&keys.signing_private_hex_seed()),
    );

    if base.tls_disable_verify {
        // above didn't include flag string; fix:
        format!(
            "moq-secure-chat-cli --relay {} --broadcast {} --track {} --tls-disable-verify \
--role subscribe --key-id {} --aead-key {} --signing-private-seed {}",
            shell_escape(&base.relay),
            shell_escape(&base.broadcast),
            shell_escape(track),
            keys.key_id,
            shell_escape(&keys.aead_key_hex()),
            shell_escape(&keys.signing_private_hex_seed()),
        )
    } else {
        format!(
            "moq-secure-chat-cli --relay {} --broadcast {} --track {} \
--role subscribe --key-id {} --aead-key {} --signing-private-seed {}",
            shell_escape(&base.relay),
            shell_escape(&base.broadcast),
            shell_escape(track),
            keys.key_id,
            shell_escape(&keys.aead_key_hex()),
            shell_escape(&keys.signing_private_hex_seed()),
        )
    }
}

fn shell_escape(s: &str) -> String {
    // Minimal escaping for copy/paste.
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Determine track
    let track = match (&cli.role, &cli.track) {
        (Role::Publish { .. }, Some(t)) => t.clone(),
        (Role::Publish { .. }, None) => random_track_hex(),
        (Role::Subscribe, Some(t)) => t.clone(),
        (Role::Subscribe, None) => anyhow::bail!("--track is required for subscribe mode"),
    };

    // Keys
    let keys = match cli.role {
        Role::Publish { .. } => {
            // If user provided overrides, use them; otherwise generate.
            if let (Some(key_id), Some(aead_key), Some(signing_seed)) =
                (cli.key_id, cli.aead_key.clone(), cli.signing_private_seed.clone())
            {
                ChatKeys::from_strings(key_id, &aead_key, &signing_seed)
                    .context("failed to construct ChatKeys from provided values")?
            } else {
                // generate defaults
                let key_id = cli.key_id;
                ChatKeys::generate(key_id).context("failed to generate ChatKeys")?
            }
        }
        Role::Subscribe => {
            // Subscriber must have aead key and a signing private seed (so we can reuse ChatKeys::decrypt_frame path).
            // (If you want, we can add a mode that uses only verify public key + aead key.)
            let key_id = cli.key_id.context("--key-id is required in subscribe mode")?;
            let aead_key = cli.aead_key.context("--aead-key is required in subscribe mode")?;
            let signing_seed = cli
                .signing_private_seed
                .as_ref()
                .or_else(|| cli.verify_public_key.as_ref())
                .cloned()
                .context("provide --signing-private-seed (preferred) or extend CLI to support verify-only")?;

            // If user passed verify_public_key, we still need the private seed for this library wiring.
            // So we require signing_private_seed for now.
            if cli.signing_private_seed.is_none() {
                anyhow::bail!("This CLI currently requires --signing-private-seed in subscribe mode (seed is used to derive verify key for MoqSecure decrypt_frame)");
            }

            ChatKeys::from_strings(key_id, &aead_key, &signing_seed)
                .context("failed to construct ChatKeys from provided values")?
        }
    };

    let url = cli.relay.clone();

    // moq-native client config with TLS disable verify
    let mut client_cfg = moq_native::ClientConfig::default();
    client_cfg.connect = url.parse().unwrap();
    client_cfg.tls = moq_native::TlsConfig {
        disable_cert_verify: cli.tls_disable_verify,
        ..Default::default()
    };

    let client = client_cfg.init()?;

    let origin = Origin::random().produce();

    match cli.role {
        Role::Publish { message } => {
            let mut broadcast = origin
                .create_broadcast(
                    &cli.broadcast,
                    moq_net::broadcast::Route::new().with_announce(true),
                )
                .context("failed to create broadcast")?;
            let track_producer = broadcast
                .create_track(track.clone(), None)
                .context("failed to create track")?;

            // Print copy-pastable subscribe command.
            let subscribe_cmd = if cli.tls_disable_verify {
                format!(
                    "moq-secure-chat-cli --relay {} --broadcast {} --track {} --tls-disable-verify --role subscribe \
--key-id {} --aead-key {} --signing-private-seed {}",
                    cli.relay,
                    cli.broadcast,
                    track,
                    keys.key_id,
                    keys.aead_key_hex(),
                    keys.signing_private_hex_seed()
                )
            } else {
                format!(
                    "moq-secure-chat-cli --relay {} --broadcast {} --track {} --role subscribe \
--key-id {} --aead-key {} --signing-private-seed {}",
                    cli.relay,
                    cli.broadcast,
                    track,
                    keys.key_id,
                    keys.aead_key_hex(),
                    keys.signing_private_hex_seed()
                )
            };

            println!("=== Copy/paste subscribe command (run in another terminal) ===");
            println!("{}", subscribe_cmd);

            let mut publisher = ChatPublisher::new(track_producer, keys);

            let reconnect = client.with_publisher(&origin).reconnect(cli.relay.clone());

            let result = tokio::select! {
                res = reconnect.closed() => res.map_err(Into::into),
                _ = async {
                    if let Some(msg) = message {
                        publisher.send_message(msg.as_bytes()).await?;
                    } else {
                        use tokio::io::{self, AsyncBufReadExt};
                        let stdin = io::stdin();
                        let mut reader = io::BufReader::new(stdin).lines();

                        tracing::info!("Reading lines from stdin; each line becomes one chat message.");
                        while let Some(line) = reader.next_line().await? {
                            publisher.send_message(line.as_bytes()).await?;
                        }
                    }
                    Ok::<(), anyhow::Error>(())
                } => Ok(()),
            };

            broadcast.finish();
            result
        }
        Role::Subscribe => {
            let reconnect = client.with_subscriber(origin.clone()).reconnect(cli.relay.clone());

            let path: Path<'_> = cli.broadcast.as_str().into();
            let mut origin = origin
                .scope(&[path])
                .context("not allowed to consume broadcast")?
                .consume()
                .announced();

            tracing::info!(broadcast = %cli.broadcast, track = %track, "waiting for broadcast to be online");

            let mut sub: Option<moq_net::track::Subscriber> = None;

            loop {
                tokio::select! {
                    Some(moq_net::announce::Update { path, broadcast }) = origin.next() => match broadcast {
                        Some(b) => {
                            tracing::info!("broadcast is online, subscribing to track");
                            let track_sub = b.track(&track)?.subscribe(None).await?;
                            sub = Some(track_sub);
                        }
                        None => {
                            tracing::warn!("broadcast offline, waiting...");
                        }
                    },
                    res = reconnect.closed() => return Ok(res?),
                    Some(_) = async { sub.is_some() } => {
                        let track_sub = sub.take().unwrap();
                        let subscriber = ChatSubscriber::new(track_sub, keys.clone());
                        subscriber.run(|pt| {
                            if let Ok(s) = String::from_utf8(pt.clone()) {
                                println!("{}", s);
                            } else {
                                eprintln!("(non-utf8 message) {} bytes", pt.len());
                            }
                        }).await?;
                    }
                }
            }
        }
    }
}
