# linux-native-video

Linux-native, low-latency video rendering experiment (GTK/Cairo).

When usable/performant, it will form the basis of a MOQ media player (including encryption/signing) for linux.  I will include this in my Moq-TV repo (mainly targeting Smart TVs at the moment).

## What this is
- A Rust project targeting Linux that uses **GTK4** and **Cairo** for rendering.
- Current pipeline is a real video decoder/player from an mp4 file.
- Next steps will use GPU textures and MOQ transport and MOQ-Secure.
- Then look at supporting multiple graphics stacks.

## Security / streaming direction
- Planned transport: **Media Over Quic (MOQ)**
- Planned payload protection: **lib moq-secure** (encrypt + sign media payloads)
- Intended architecture: streaming layer (MOQ) → verified/decrypted media chunks → decoder → renderer.

## Current status (NOT fully usable yet)
This project is **under construction / experimentation**:
 ### Working:
 - Video frame decoding and rendering.
 ### Not Yet Working:
 - No integration with FFmpeg / hardware decode (VAAPI) yet.
 - No MOQ playback pipeline yet.

## Run
From main repo root:
```bash
cargo build -p linux-native-video
```
You need a copy of bbb.mp4 (Big Buck Bunny) in same directory as binary.
When built (assuming bbb.mp4 is in directory):
```bash
cd target/debug
./linux-native-video
```
