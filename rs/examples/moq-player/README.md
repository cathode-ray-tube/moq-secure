# moq-player

Linux-native, low-latency video rendering experiment.

This forms the basis of a MoQ media player (including encryption/signing) for linux.  I will include this in my [moq-tv](https://github.com/cathode-ray-tube/moq-tv) repo (mainly targeting Smart TVs at the moment).

## What this is
- A Rust project targeting Linux that uses **GTK4** and **GStreamer**.
- Current pipeline is a real video decoder/player, connecting to a MoQ Relay and playing audio/video.

## Security / streaming direction
- Transport: **Media over Quic (MoQ)**
- Planned payload protection: **moq-secure** (encrypt + sign media payloads)
- Intended architecture: streaming layer (MoQ) → verified/decrypted media chunks → decoder → renderer.

## Current status
 - Plays MoQ audio and video.
 - MOQ-Secure not yet added.

## Prerequisites

Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Run
From main repo root:
```bash
cargo install moq-player
```
When installed:
```bash
moq-player
```
