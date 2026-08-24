# linux-native-video

Linux-native, low-latency video rendering experiment.

When usable/performant, it will form the basis of a MOQ media player (including encryption/signing) for linux.  I will include this in my Moq-TV repo (mainly targeting Smart TVs at the moment).

## What this is
- A Rust project targeting Linux that uses **GTK4** and **GStreamer**.
- Current pipeline is a real video decoder/player, connecting to a MOQ Relay and playing audio/video.

## Security / streaming direction
- Planned transport: **Media Over Quic (MOQ)**
- Planned payload protection: **lib moq-secure** (encrypt + sign media payloads)
- Intended architecture: streaming layer (MOQ) → verified/decrypted media chunks → decoder → renderer.

## Current status
MOQ-Secure not yet added.

### Working:
 - Audio and video decoding and rendering.
 - MOQ playback using GStreamer plugin.

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
