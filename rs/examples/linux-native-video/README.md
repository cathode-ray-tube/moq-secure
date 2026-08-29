# linux-native-video

Linux-native, low-latency video rendering experiment.

When usable/performant, it will form the basis of a MOQ media player (including encryption/signing) for linux.  I will include this in my [moq-tv](https://github.com/cathode-ray-tube/moq-tv) repo (mainly targeting Smart TVs at the moment).

## What this is
- A Rust project targeting Linux that uses **GTK4** and **GStreamer**.
- Current pipeline is a real video decoder/player, connecting to a MOQ Relay and playing audio/video.

## Security / streaming direction
- Transport: **Media Over Quic (MOQ)**
- Planned payload protection: **moq-secure** (encrypt + sign media payloads)
- Intended architecture: streaming layer (MOQ) → verified/decrypted media chunks → decoder → renderer.

## Current status
MOQ-Secure not yet added.

### Working:
 - Audio and video decoding and rendering.
 - MOQ playback using GStreamer plugin.

## Prerequisites

Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Run
From main repo root:
```bash
cargo build -p linux-native-video
```
When built:
```bash
cd target/debug
./linux-native-video
```
