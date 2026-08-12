use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use moq_net::{Origin, Path};
use moq_secure_chat::{ChatKeys, ChatPublisher, ChatSubscriber};
use rand::RngCore;
use url::Url;

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

    // Keys: generate defaults when not supplied (publish-focused behavior).
    let key_id: u8 = cli.key_id.unwrap_or_else(gen_key_id);
    let aead_key: String = cli.aead_key.unwrap_or_else(gen_aead_key_hex);

    // moq-native client config with TLS disable verify
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
        Role::Publish { message } => {
            // Publish needs signing private seed for ChatKeys::from_strings.
            // If not provided, generate one.
            let signing_private_seed_or_bytes = cli
                .signing_private_seed
                .unwrap_or_else(gen_signing_private_seed_hex);

            let keys = ChatKeys::from_strings(key_id, &aead_key, &signing_private_seed_or_bytes)
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

            // Print copy-pastable subscribe command.
            // Subscribe mode now accepts ONLY --signing-public-key (plus transport args).
            let subscribe_cmd = if cli.tls_disable_verify {
                format!(
                    "moq-secure-chat-cli --relay {} --broadcast {} --track {} --tls-disable-verify --role subscribe \
--key-id {} --aead-key {} --signing-public-key {}",
                    shell_escape(&cli.relay),
                    shell_escape(&cli.broadcast),
                    shell_escape(&track),
                    keys.key_id,
                    shell_escape(&keys.aead_key_hex()),
                    shell_escape(&keys.signing_verify_hex())
                )
            } else {
                format!(
                    "moq-secure-chat-cli --relay {} --broadcast {} --track {} --role subscribe \
--key-id {} --aead-key {} --signing-public-key {}",
                    shell_escape(&cli.relay),
                    shell_escape(&cli.broadcast),
                    shell_escape(&track),
                    keys.key_id,
                    shell_escape(&keys.aead_key_hex()),
                    shell_escape(&keys.signing_verify_hex())
                )
            };

            println!("=== Copy/paste subscribe command (run in another terminal) ===");
            println!("{subscribe_cmd}");

            let mut publisher = ChatPublisher::new(track_producer, keys);

            let reconnect = client
                .with_publisher(&origin)
                .reconnect(relay_url);

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
                } => Ok(())
            };

            broadcast.finish();
            result
        }

        Role::Subscribe => {
            // Subscribe needs signing public verify key (cannot be derived without private key).
            let signing_public = cli
                .signing_public_key
                .context("--signing-public-key is required for subscribe mode")?;

            // Construct ChatKeys using the public-verify constructor (dummy private inside).
            let keys = ChatKeys::from_strings_public_verify(key_id, &aead_key, &signing_public)
                .context("failed to construct ChatKeys (public-verify)")?;

            let reconnect = client
                .with_subscriber(origin.clone())
                .reconnect(relay_url);

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

            let mut sub: Option<moq_net::track::Subscriber> = None;

            loop {
                tokio::select! {
                    Some(moq_net::announce::Update { path: _path, broadcast }) = origin.next() => match broadcast {
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
                    true = async { sub.is_some() } => {
                        let track_sub = sub.take().unwrap();
                        let subscriber = ChatSubscriber::new(track_sub, keys.clone());
                        subscriber
                            .run(|pt| {
                                if let Ok(s) = String::from_utf8(pt.clone()) {
                                    println!("{}", s);
                                } else {
                                    eprintln!("(non-utf8 message) {} bytes", pt.len());
                                }
                            })
                            .await?;
                    }
                }
            }
        }
    }
}
