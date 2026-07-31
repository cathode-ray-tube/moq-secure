use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ed25519_dalek::{SigningKey, VerifyingKey};
use moq_native::moq_net::origin::Producer;
use moq_native::moq_net::*;
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};
use url::Url;

use moq_secure::{decrypt_frame, encrypt_frame};
use moq_secure::error::MoqSecureError;

const DEFAULT_KEY_ID: u8 = 0;
const DEFAULT_CTR_START: u64 = 1;
const DEFAULT_NGROUPS: u8 = 1; // informational; we just use one group per message

fn parse_key32(s: &str, label: &str) -> Result<[u8; 32]> {
    let ss = s.trim();

    // Hex (64 chars)
    if ss.len() == 64 && ss.chars().all(|c| c.is_ascii_hexdigit()) {
        let bytes = hex::decode(ss).with_context(|| format!("decoding hex {label}"))?;
        return Ok(bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("{label} hex decoded to wrong length"))?);
    }

    // Base64
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(ss)
        .with_context(|| format!("decoding base64 {label}"))?;
    Ok(bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("{label} base64 decoded to wrong length"))?)
}

fn parse_signing_key(s: &str) -> Result<SigningKey> {
    // 32-byte ed25519 seed expected
    let seed = parse_key32(s, "signingKey")?;
    Ok(SigningKey::from_bytes(&seed))
}

fn parse_verify_key(s: &str) -> Result<VerifyingKey> {
    // 32-byte public key expected
    let pk = parse_key32(s, "verifyKey")?;
    Ok(VerifyingKey::from_bytes(&pk))
}

#[derive(Parser, Debug)]
#[command(name = "moq-secure-chat-cli")]
#[command(about = "MoQ chat using moq-secure to encrypt+sign every frame")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run an interactive publisher (sender).
    Publish {
        /// WebTransport/QUIC connect URL
        #[arg(long)]
        url: String,

        /// MoQ broadcast route/path name
        #[arg(long, default_value = "chat-example")]
        broadcast: String,

        /// MoQ track name
        #[arg(long, default_value = "chat")]
        track: String,

        /// Shared symmetric key (ChaCha20-Poly) as 32 bytes: hex(64) or base64
        #[arg(long)]
        encryptionKey: String,

        /// Ed25519 private signing key (seed) as 32 bytes: hex(64) or base64
        #[arg(long)]
        signingKey: String,
    },

    /// Run a subscriber that decrypts+verifies and prints messages.
    Subscribe {
        /// WebTransport/QUIC connect URL
        #[arg(long)]
        url: String,

        /// MoQ broadcast route/path name
        #[arg(long, default_value = "chat-example")]
        broadcast: String,

        /// MoQ track name
        #[arg(long, default_value = "chat")]
        track: String,

        /// Shared symmetric key (same as publisher): hex(64) or base64
        #[arg(long)]
        encryptionKey: String,

        /// Publisher ed25519 public key (verify key): hex(64) or base64
        #[arg(long)]
        verifyKey: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Command::Publish {
            url,
            broadcast,
            track,
            encryptionKey,
            signingKey,
        } => publish(&url, &broadcast, &track, &encryptionKey, &signingKey).await,

        Command::Subscribe {
            url,
            broadcast,
            track,
            encryptionKey,
            verifyKey,
        } => subscribe(&url, &broadcast, &track, &encryptionKey, &verifyKey).await,
    }
}

async fn publish(
    url_str: &str,
    broadcast: &str,
    track: &str,
    encryption_key_str: &str,
    signing_key_str: &str,
) -> Result<()> {
    let encryption_key_32 =
        parse_key32(encryption_key_str, "encryptionKey").context("bad encryptionKey")?;
    let signing_key = parse_signing_key(signing_key_str).context("bad signingKey")?;
    let verify_key = signing_key.verifying_key();

    let mut keys: [[u8; 32]; 256] = [[0u8; 32]; 256];
    keys[DEFAULT_KEY_ID as usize] = encryption_key_32;

    let url = Url::parse(url_str).context("bad --url")?;

    // Create origin (publisher).
    let origin = moq_native::moq_net::Origin::random().produce();

    // Create broadcast + track.
    let mut broadcast_obj = origin
        .create_broadcast(broadcast, moq_native::moq_net::broadcast::Route::new().with_announce(true))
        .context("failed to create broadcast")?;

    let mut track_pub = broadcast_obj
        .create_track(track, None)
        .context("failed to create track")?;

    // Copy/paste subscriber command.
    let verify_hex = hex::encode(verify_key.as_bytes());

    println!("\nSubscriber command:\nmoq-secure-chat-cli subscribe --url '{url_str}' --broadcast '{broadcast}' --track '{track}' --encryptionKey '{encryption_key_str}' --verifyKey {verify_hex}\n");

    let client = moq_native::ClientConfig::default().init()?;
    let _cs = client
        .with_publisher(&origin)
        .connect(url)
        .await
        .context("publisher connect failed")?;

    println!("Publisher ready. Type messages and press Enter. Ctrl+C to quit.");

    let mut ctr: u64 = DEFAULT_CTR_START;

    // One frame per line from stdin.
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    // Signing every frame: n_signed = 1
    let n_signed: u8 = 1;

    while let Some(line) = lines.next_line().await? {
        let msg = line.trim_end();
        if msg.is_empty() {
            print!("> ");
            io::stdout().flush().ok();
            continue;
        }

        let plaintext = msg.as_bytes();

        let ef = encrypt_frame(
            &keys,
            &signing_key,
            DEFAULT_KEY_ID,
            ctr,
            n_signed,
            true, // maybe_sign; with n_signed=1 it will sign every frame
            plaintext,
        );

        // Send as the MoQ track frame payload.
        track_pub.write_frame(
            moq_native::moq_net::Timestamp::now(),
            bytes::Bytes::from(ef.serialize()),
        )?;

        ctr = ctr.wrapping_add(1);

        print!("> ");
        io::stdout().flush().ok();
    }

    broadcast_obj.finish();
    Ok(())
}

async fn subscribe(
    url_str: &str,
    broadcast: &str,
    track: &str,
    encryption_key_str: &str,
    verify_key_str: &str,
) -> Result<()> {
    let encryption_key_32 =
        parse_key32(encryption_key_str, "encryptionKey").context("bad encryptionKey")?;
    let verify_key =
        parse_verify_key(verify_key_str).context("bad verifyKey")?;

    let mut keys: [[u8; 32]; 256] = [[0u8; 32]; 256];
    keys[DEFAULT_KEY_ID as usize] = encryption_key_32;

    let url = Url::parse(url_str).context("bad --url")?;
    let client = moq_native::ClientConfig::default().init()?;

    let origin = moq_native::moq_net::Origin::random().produce();
    let reconnect = client.with_subscriber(origin.clone()).reconnect(url);

    println!("Subscriber: waiting for broadcast to be online…");

    let path: moq_native::moq_net::Path<'_> = broadcast.into();
    let mut origin = origin
        .scope(&[path])
        .context("not allowed to consume broadcast")?
        .consume()
        .announced();

    let mut lease_remaining: u8 = 0;

    let mut track_sub: Option<moq_native::moq_net::track::Subscriber> = None;

    loop {
        tokio::select! {
            Some(moq_native::announce::Update { path, broadcast }) = origin.next() => {
                if let Some(b) = broadcast {
                    let _ = path;
                    println!("Broadcast online; subscribing to track…");
                    // Subscribe to track within the broadcast.
                    let t = b.track(&track)
                        .context("track not in broadcast")?
                        .subscribe(None).await
                        .context("failed to subscribe track")?;
                    track_sub = Some(t);
                } else {
                    println!("Broadcast offline; waiting…");
                }
            }
            res = reconnect.closed() => {
                // connection closed
                res.map_err(|e| anyhow::anyhow!("subscriber closed: {e:?}"))?;
                return Ok(());
            }
            // If we have an active track subscription, run its receive loop one group at a time.
            Some(result) = async {
                if let Some(ref mut tsub) = track_sub {
                    // Receive the next group; this will await until one arrives.
                    let grp = tsub.recv_group().await.ok()?;
                    Some(grp)
                } else {
                    None
                }
            } => {
                let mut group = match result {
                    Some(g) => g,
                    None => continue,
                };

                while let Some(frame) = group.read_frame().await.ok()? {
                    let payload = frame.payload;

                    match decrypt_frame(
                        &keys,
                        &verify_key,
                        &mut lease_remaining,
                        &payload,
                    ) {
                        Ok(plaintext) => {
                            let text = String::from_utf8_lossy(&plaintext);
                            println!("{text}");
                        }
                        Err(e) => {
                            // keep going; show error
                            match e {
                                MoqSecureError::InvalidSignature => eprintln!("Frame rejected: invalid signature"),
                                MoqSecureError::AeadAuthFailed => eprintln!("Frame rejected: AEAD auth failed"),
                                other => eprintln!("Frame rejected: {other:?}"),
                            }
                        }
                    }
                }
            }
        }
    }
}
