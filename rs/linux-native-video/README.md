# linux-native-video

Linux-native, low-latency video rendering experiment (GTK + OpenGL).

When usable/performant, it will form the basis of a MOQ media player (including encryption/signing) for linux.  I will include this in my Moq-TV repo (mainly targeting Smart TVs at the moment).

## What this is
- A Rust project targeting Linux that uses **GTK4** and **GLArea** as the rendering surface.
- Current pipeline is **not** a real video decoder/player yet—GL rendering is only placeholder code.
- Next steps will replace the placeholder rendering with actual decoded frames (e.g., FFmpeg/VAAPI) and upload them to GPU textures.

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
