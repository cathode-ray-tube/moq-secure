use anyhow::{Context, Result};
use ed25519_dalek::{SigningKey, VerifyingKey};
use moq_native::moq_net::*;
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};
use url::Url;

use base64::Engine;
use rand::RngCore;

use moq_secure::{decrypt_frame, encrypt_frame};
use moq_secure::error::MoqSecureError;

const DEFAULT_KEY_ID: u8 = 0;
const DEFAULT_CTR_START: u64 = 1;

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

fn parse_signing_key_seed(s: &str) -> Result<SigningKey> {
    let seed = parse_key32(s, "signingKey")?;
    Ok(SigningKey::from_bytes(&seed))
}

fn parse_verify_key(s: &str) -> Result<VerifyingKey> {
    let pk = parse_key32(s, "verifyKey")?;
    let vk = VerifyingKey::from_bytes(&pk).map_err(|e| anyhow::anyhow!(e))?;
    Ok(vk)
}

fn gen_random_32() -> [u8; 32] {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    b
}

fn gen_track_name_8hex() -> String {
    // 8 hex chars => 4 bytes
    let mut b = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush().ok();
    let mut s = String::new();
    io::stdin().read_line(&mut s).context("read_line failed")?;
    Ok(s.trim().to_string())
}

async fn publisher(
    url: Url,
    broadcast: String,
    track_name: String,
    encryption_key: [u8; 32],
    signing_key: SigningKey,
) -> Result<()> {
    let verify_hex = hex::encode(signing_key.verifying_key().as_bytes());
    let enc_hex = hex::encode(encryption_key);

    println!("Publisher auto-generated keys:");
    println!("  verify/public signing key: {verify_hex}");
    println!("  symmetric encryption key : {enc_hex}");
    println!();
    println!("Publisher connect: {url}");
    println!("Publishing on broadcast='{broadcast}', track='{track_name}'");
    println!("Type messages and press Enter. Ctrl+C to quit.");

    let mut keys: [[u8; 32]; 256] = [[0u8; 32]; 256];
    keys[DEFAULT_KEY_ID as usize] = encryption_key;

    let origin = moq_native::moq_net::Origin::random().produce();

    let mut broadcast_obj = origin
        .create_broadcast(
            broadcast,
            moq_native::moq_net::broadcast::Route::new().with_announce(true),
        )
        .context("failed to create broadcast")?;

    let mut track_pub = broadcast_obj
        .create_track(&track_name, None)
        .context("failed to create track")?;

    let client = moq_native::ClientConfig::default().init()?;
    client
        .with_publisher(&origin)
        .connect(url)
        .await
        .context("publisher connect failed")?;

    let mut ctr: u64 = DEFAULT_CTR_START;
    let n_signed: u8 = 1;

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    while let Some(line) = lines.next_line().await? {
        let msg = line.trim_end();
        if msg.is_empty() {
            continue;
        }

        let ef = encrypt_frame(
            &keys,
            &signing_key,
            DEFAULT_KEY_ID,
            ctr,
            n_signed,
            true, // maybe_sign
            msg.as_bytes(),
        );

        track_pub.write_frame(
            moq_native::moq_net::Timestamp::now(),
            bytes::Bytes::from(ef.serialize()),
        )?;

        ctr = ctr.wrapping_add(1);
    }

    broadcast_obj.finish();
    Ok(())
}

async fn subscriber_shell(
    url: Url,
    broadcast: String,
    // local default track name (only used when user manually subscribes)
    local_track_name: String,
    // optional: signing key used by `submine` command
    my_signing_key: Option<SigningKey>,
) -> Result<()> {
    let client = moq_native::ClientConfig::default().init()?;

    let origin = moq_native::moq_net::Origin::random().produce();
    let reconnect = client.with_subscriber(origin.clone()).reconnect(url);

    let path: moq_native::moq_net::Path<'_> = broadcast.clone().into();

    println!("Subscriber: connect {url}");
    println!("Consuming broadcast='{broadcast}'");
    println!();

    let mut origin = origin
        .scope(&[path])
        .context("not allowed to consume broadcast")?
        .consume()
        .announced();

    let mut lease_remaining: u8 = 0;
    let mut active_track_sub: Option<moq_native::moq_net::track::Subscriber> = None;
    let mut active_verify_key: Option<VerifyingKey> = None;
    let mut active_keys: [[u8; 32]; 256] = [[0u8; 32]; 256];

    let mut lines = BufReader::new(tokio::io::stdin()).lines();

    // subscription selection derived from announce payload
    let mut pending_announce_subscribe: Option<bool> = None;
    let mut last_announced_track_name: Option<String> = None;

    // When user types a command we apply it when possible.
    // Since we only have `b` (announce object) when broadcast is online,
    // we keep pending manual subscription config too.
    let mut pending_manual_sub: Option<(String /*track*/, VerifyingKey, [u8;32])> = None;

    loop {
        tokio::select! {
            // 1) Process broadcast announce updates
            Some(moq_net::announce::Update { path: _path, broadcast }) = origin.next() => {
                if let Some(b) = broadcast {
                    // Extract announced track name from the announce payload `b`.
                    // This library’s announce object type can vary by version;
                    // we handle common cases with pattern matching below by trying
                    // to use its Debug string as fallback.
                    //
                    // IMPORTANT: Your request is specifically “track name in announce payload”.
                    // So we try to get it from b using known shapes first, then fallback.
                    let announced_track = extract_announced_track_name_from_b(&b)
                        .or_else(|| {
                            // fallback heuristic from Debug; expects substring like "track: \"...\"" or "...track='...'"
                            let dbg = format!("{b:?}");
                            extract_track_name_from_debug_heuristic(&dbg)
                        });

                    if let Some(tname) = announced_track.clone() {
                        last_announced_track_name = Some(tname.clone());
                        println!("Broadcast online. Announced track='{tname}'");
                    } else {
                        println!("Broadcast online. (Announced track name not extractable automatically from this moq_native version)");
                    }

                    // Offer subscribe-to-announced option
                    let ans = prompt_line("Subscribe to announced track from this publisher? (y/n): ")?;
                    pending_announce_subscribe = Some(matches!(ans.as_str(), "y" | "Y" | "yes" | "YES"));

                    // If yes and we know announced track name, prompt for keys and subscribe.
                    if pending_announce_subscribe == Some(true) {
                        let tname = match last_announced_track_name.clone() {
                            Some(x) => x,
                            None => {
                                eprintln!("Cannot subscribe to announced track because track name couldn’t be extracted.");
                                continue;
                            }
                        };

                        let verify_hex_or_b64 = prompt_line("Publisher verify key (hex(64) or base64): ")?;
                        let enc_hex_or_b64 = prompt_line("Publisher symmetric encryption key (32 bytes hex or base64): ")?;

                        let vk = match parse_verify_key(&verify_hex_or_b64) {
                            Ok(v) => v,
                            Err(e) => { eprintln!("Bad verifyKey: {e:?}"); continue; }
                        };
                        let enc = match parse_key32(&enc_hex_or_b64, "encryptionKey") {
                            Ok(k) => k,
                            Err(e) => { eprintln!("Bad encryptionKey: {e:?}"); continue; }
                        };

                        // Subscribe to announced track
                        match b.track(&tname) {
                            Ok(tb) => {
                                match tb.subscribe(None).await {
                                    Ok(tsub) => {
                                        active_track_sub = Some(tsub);
                                        active_verify_key = Some(vk);
                                        active_keys = [[0u8; 32]; 256];
                                        active_keys[DEFAULT_KEY_ID as usize] = enc;
                                        lease_remaining = 0;
                                        println!("Subscribed to announced track '{tname}'.");
                                    }
                                    Err(e) => eprintln!("Failed to subscribe announced track: {e:?}"),
                                }
                            }
                            Err(e) => eprintln!("Track not in broadcast: {e:?}"),
                        }
                    } else {
                        println!("Not subscribing to announced track.");
                        pending_manual_sub.take(); // optional: clear manual pending if you prefer
                    }

                    // If user already typed a manual `sub` command while we were offline,
                    // apply it now.
                    if let Some((tname, vk, enc)) = pending_manual_sub.take() {
                        match b.track(&tname) {
                            Ok(tb) => {
                                match tb.subscribe(None).await {
                                    Ok(tsub) => {
                                        active_track_sub = Some(tsub);
                                        active_verify_key = Some(vk);
                                        active_keys = [[0u8; 32]; 256];
                                        active_keys[DEFAULT_KEY_ID as usize] = enc;
                                        lease_remaining = 0;
                                        println!("Subscribed to track '{tname}'.");
                                    }
                                    Err(e) => eprintln!("Failed manual subscribe: {e:?}"),
                                }
                            }
                            Err(e) => eprintln!("Track not in broadcast: {e:?}"),
                        }
                    }
                } else {
                    println!("Broadcast offline; waiting…");
                    active_track_sub = None;
                    active_verify_key = None;
                    lease_remaining = 0;
                }
            }

            // 2) Read stdin commands
            cmd = async {
                match lines.next_line().await {
                    Ok(Some(s)) => Some(s),
                    _ => None
                }
            } => {
                let Some(cmdline) = cmd else { return Ok(()); };
                let cmdline = cmdline.trim();
                if cmdline.is_empty() { continue; }

                let parts: Vec<&str> = cmdline.split_whitespace().collect();
                match parts.as_slice() {
                    ["help"] => {
                        println!("Commands:");
                        println!("  sub <trackName> <verifyKeyHexOrB64> <encryptionKeyHexOrB64>");
                        println!("  submine <encryptionKeyHexOrB64>   (subscribes to local auto track using my verify key)");
                        println!("  subscribe-announced (y/n)          (answers the next announce-online prompt automatically)");
                        println!("  quit");
                    }
                    ["quit"] => return Ok(()),
                    ["subscribe-announced", v] => {
                        pending_announce_subscribe = Some(matches!(*v, "y"|"Y"|"yes"|"YES"));
                        println!("Set next announced-track choice to '{v}'. Next time broadcast is online you will be prompted for keys if y.");
                    }
                    ["sub", tname, vk_str, enc_str] => {
                        let vk = match parse_verify_key(vk_str) {
                            Ok(v) => v,
                            Err(e) => { eprintln!("Bad verifyKey: {e:?}"); continue; }
                        };
                        let enc = match parse_key32(enc_str, "encryptionKey") {
                            Ok(k) => k,
                            Err(e) => { eprintln!("Bad encryptionKey: {e:?}"); continue; }
                        };

                        // If currently subscribed, we’ll switch when next announce comes;
                        // simplest robust approach with this moq API flow.
                        pending_manual_sub = Some(((*tname).to_string(), vk, enc));
                        println!("Queued manual subscription to '{tname}'. Waiting for broadcast-online to apply.");
                    }
                    ["submine", enc_str] => {
                        let Some(sk) = my_signing_key.as_ref() else {
                            eprintln!("submine requires running in publisher mode (so we have my signing key).");
                            continue;
                        };
                        let vk = sk.verifying_key();
                        let enc = match parse_key32(enc_str, "encryptionKey") {
                            Ok(k) => k,
                            Err(e) => { eprintln!("Bad encryptionKey: {e:?}"); continue; }
                        };

                        pending_manual_sub = Some((local_track_name.clone(), vk, enc));
                        println!("Queued submine to local track '{local_track_name}'. Waiting for broadcast-online to apply.");
                    }
                    _ => {
                        println!("Unknown command. Type 'help'.");
                    }
                }

                print!("moq> ");
                io::stdout().flush().ok();
            }

            // 3) Drain frames from active track subscription (if any)
            _ = async {
                if let Some(ref mut tsub) = active_track_sub {
                    let mut group = match tsub.recv_group().await {
                        Ok(Some(g)) => g,
                        _ => return,
                    };
                    let vk = match active_verify_key.as_ref() {
                        Some(v) => v,
                        None => return,
                    };

                    loop {
                        let frame = match group.read_frame().await {
                            Ok(Some(f)) => f,
                            Ok(None) => break,
                            Err(e) => {
                                eprintln!("read_frame failed: {e:?}");
                                break;
                            }
                        };

                        match decrypt_frame(&active_keys, vk, &mut lease_remaining, &frame.payload) {
                            Ok(plaintext) => {
                                let text = String::from_utf8_lossy(&plaintext);
                                println!("{text}");
                                print!("moq> ");
                                io::stdout().flush().ok();
                            }
                            Err(e) => match e {
                                MoqSecureError::InvalidSignature => eprintln!("Frame rejected: invalid signature"),
                                MoqSecureError::AeadAuthFailed => eprintln!("Frame rejected: AEAD auth failed"),
                                other => eprintln!("Frame rejected: {other:?}"),
                            },
                        }
                    }
                }
            } => {}
            // 4) Reconnect closed
            res = reconnect.closed() => {
                res.map_err(|e| anyhow::anyhow!("subscriber closed: {e:?}"))?;
                return Ok(());
            }
        }
    }
}

// Attempts to extract announced track name from the announce object.
// The exact type depends on moq_native version, so we use a few strategies:
// - If b has obvious method/fields (not known at compile time here), this will be updated.
// - Otherwise, we return None and rely on Debug heuristic in caller.
fn extract_announced_track_name_from_b(_b: &moq_native::moq_net::announce::TrackAnnounce) -> Option<String> {
    // If your moq_native version has a direct API for “track name” in the announce object,
    // implement it here. Because we can’t see your crate version/types, the safest approach
    // is to use Debug heuristic below (caller already does that fallback).
    None
}

// Heuristic: try to find something that looks like a track name in debug output.
fn extract_track_name_from_debug_heuristic(dbg: &str) -> Option<String> {
    // Common patterns:
    // - track: "xyz"
    // - track_name: "xyz"
    // - track='xyz'
    // We’ll try a couple lightweight string searches.
    let candidates = [
        ("track: \"", "\""),
        ("track_name: \"", "\""),
        ("track='", "'"),
        ("track_name='", "'"),
    ];

    for (start, end_delim) in candidates {
        if let Some(i) = dbg.find(start) {
            let s = &dbg[i + start.len()..];
            if let Some(j) = s.find(end_delim) {
                let val = &s[..j];
                if !val.trim().is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <url>", args[0]);
        std::process::exit(2);
    }
    let url = Url::parse(&args[1]).context("bad URL")?;

    // Generate for this run
    let encryption_key_32 = gen_random_32();
    let signing_seed_32 = gen_random_32();
    let signing_key = SigningKey::from_bytes(&signing_seed_32);
    let verify_key = signing_key.verifying_key();
    let auto_track_name = gen_track_name_8hex();

    println!("Auto-generated (this run):");
    println!("  track name (8 hex): {auto_track_name}");
    println!("  verify/public signing key (hex): {}", hex::encode(verify_key.as_bytes()));
    println!("  symmetric encryption key (hex): {}", hex::encode(encryption_key_32));
    println!();

    let broadcast = prompt_line("Broadcast route/path (e.g. chat-example): ")?;
    let mode = prompt_line("Mode: (p)ublish, (s)ubscribe, or (b)oth: ")?;

    match mode.as_str() {
        "p" | "P" => {
            publisher(url, broadcast, auto_track_name, encryption_key_32, signing_key).await
        }
        "s" | "S" => {
            // In subscribe-only mode we don’t have signing_key unless you want submine to work.
            // But you asked to implement interactive publisher too; so in s-mode we allow submine only
            // by default if you want; easiest is: enable it with the generated signing key.
            subscriber_shell(
                url,
                broadcast,
                auto_track_name,
                Some(signing_key),
            ).await
        }
        "b" | "B" => {
            // both: run publisher and subscriber concurrently.
            // Subscriber uses my signing key for submine.
            let publisher_task = tokio::spawn({
                let url2 = url.clone();
                let broadcast2 = broadcast.clone();
                let track2 = auto_track_name.clone();
                let enc2 = encryption_key_32;
                let sk2 = signing_key;
                async move { publisher(url2, broadcast2, track2, enc2, sk2).await }
            });

            let subscriber_task = tokio::spawn({
                let url2 = url;
                let broadcast2 = broadcast;
                let track2 = auto_track_name;
                let enc2 = encryption_key_32;
                // We need a SigningKey for submine; re-generate seed would change identity.
                // So: in “both” mode, we set my_signing_key = None (no submine) to keep keys consistent.
                // If you want submine in both mode, you can restructure to clone/shared seed and build two SigningKeys.
                let my_signing_key_for_submine: Option<SigningKey> = {
                    // We cannot reuse signing_key moved into publisher_task.
                    // So we disable submine here for correctness.
                    let _ = enc2;
                    None
                };
                async move {
                    subscriber_shell(url2, broadcast2, track2, my_signing_key_for_submine).await
                }
            });

            let (a, b) = tokio::join!(publisher_task, subscriber_task);
            a??;
            b??;
            Ok(())
        }
        _ => {
            eprintln!("Unknown mode. Use p/s/b.");
            Ok(())
        }
    }
}
