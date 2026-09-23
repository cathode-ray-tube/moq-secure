![moq-secure logo](./assets/moq-secure-logo-256.png)

[![JavaScript Tests](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-js.yml/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-js.yml)
[![Rust Tests](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-rust.yml/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-rust.yml)
[![CodeQL](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/github-code-scanning/codeql)
![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)
[![crates.io version](https://img.shields.io/crates/v/moq-secure.svg)](https://crates.io/crates/moq-secure)
[![Downloads](https://img.shields.io/crates/d/moq-secure.svg)](https://crates.io/crates/moq-secure)
[![docs.rs](https://img.shields.io/docsrs/moq-secure)](https://docs.rs/moq-secure)

# MOQ-Secure Encryption & Signing

A fixed format for end-to-end encrypting **media payloads** carried by **Media Over QUIC (MOQ)** using **AEAD encryption (ChaCha20-Poly1305)** with an **optional Ed25519 signature**.

> **Payload-Only Encryption:** MOQ is a content-agnostic transport format. MOQ-Secure encrypts only the frame’s media payload bytes. Transport framing and routing remain unchanged. **Metadata such as broadcast name, media codec and resolution remains unencrypted and visible to the relay.**

## Purpose

People increasingly want to protect their communications from pervasive monitoring and mass surveillance. At the same time, audiences need confidence that media is genuine: in an era of deepfakes, you often can’t tell whether a video or audio clip truly came from the person it claims to be.

MOQ-Secure is designed to provide:
- **Privacy** for the media payload - so content can’t be inspected in transit
- **Integrity** - so tampering is detected
- **Authenticity** - so frames can be verified as coming from a particular publisher
- **Flexibility** - balancing security and performance

Publishers are able to use **any** public MOQ CDN, with the hosting provider unable to see the content. Consumers can verify the publisher of the content, wherever they receive it from.

## Quick Start (moq-secure-chat-cli):

This demonstrates moq-secure end-to-end encryption and signing of text chat messages in the terminal.

### Prerequisites

Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Install **moq-relay** (the server) from the moq repo, full instructions [here](https://github.com/moq-dev/moq/tree/main/rs/moq-relay).

### Run

Run moq-relay with this config file, [localhost.toml](https://github.com/moq-dev/moq/blob/main/demo/relay/localhost.toml):

```bash
wget https://raw.githubusercontent.com/moq-dev/moq/refs/heads/main/demo/relay/localhost.toml
moq-relay localhost.toml
```

In a **2nd terminal**, install the binary:

```bash
cargo install moq-secure-chat-cli
```

Run:

```bash
moq-secure-chat-cli --relay https://localhost:4443/chat --tls-disable-verify publish
```

In a **3rd terminal**, paste the displayed command to run the binary in subscriber mode and receive sent messages.

### Troubleshooting

Run binary with `-h` or `--help` flag to list available args and usage:

```bash
moq-secure-chat-cli --help
```

### Production

This is a demo app, with usability prioritized. For production, hardening would be required, particularly around key management and distribution.

## Quick Start (moq-player):

![moq-player-screenshot](https://raw.githubusercontent.com/cathode-ray-tube/moq-secure/main/assets/moq-player.jpg)

[![crates.io version](https://img.shields.io/crates/v/moq-player.svg)](https://crates.io/crates/moq-player)
[![Downloads](https://img.shields.io/crates/d/moq-player.svg)](https://crates.io/crates/moq-player)
[![docs.rs](https://img.shields.io/docsrs/moq-player)](https://docs.rs/moq-player)

*Ubuntu or Debian only*

This is a linux-native player app using GStreamer and GTK4. MOQ-Secure is not yet integrated. Open a terminal and paste the following commands:

### 1. Install system dependencies

```bash
sudo apt update

sudo apt install -y \
  build-essential \
  curl \
  pkg-config \
  libssl-dev \
  libgtk-4-dev \
  libgstreamer1.0-dev \
  libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-tools \
  gstreamer1.0-plugins-base \
  gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad \
  gstreamer1.0-libav \
  gstreamer1.0-gtk4
```

### 2. Install the MoQ GStreamer plugin

```bash
curl -fsSL https://apt.moq.dev/moq-keyring.gpg \
  | sudo tee /usr/share/keyrings/moq-keyring.gpg >/dev/null

echo "deb [signed-by=/usr/share/keyrings/moq-keyring.gpg] https://apt.moq.dev stable main" \
  | sudo tee /etc/apt/sources.list.d/moq.list >/dev/null

sudo apt update
sudo apt install -y gstreamer1.0-moq
```

Check that the MoQ plugin is installed:

```bash
gst-inspect-1.0 moq
```

The output should include `moqsrc` and `moqsink`.

### 3. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

When prompted, press Enter to choose the default installation.

Then load Rust into the current terminal:

```bash
source "$HOME/.cargo/env"
```

### 4. Install moq-player

```bash
cargo install moq-player
```

### 5. Run moq-player

```bash
moq-player
```

A window should open, playing `Big Buck Bunny`.

## Specification

The complete field layout, byte concatenation rules, nonce/AAD/digest definitions, and receiver processing order are in [specification](https://github.com/cathode-ray-tube/moq-secure/blob/main/specification/README.md).

Quick reference [format](https://github.com/cathode-ray-tube/moq-secure/blob/main/frame_format.txt).

## Interoperability

Only encrypts the payload so it should work with any MOQ implementation (moq-lite, IETF implementations, etc.).

While aimed at MOQ, with some additional wiring it could encrypt and sign data sent via other transports (such as WebSockets, WebRTC Data Channels, etc).

This repo contains implementations in [rust](https://github.com/cathode-ray-tube/moq-secure/tree/main/rs) and [javascript](https://github.com/cathode-ray-tube/moq-secure/tree/main/js). Compatability considerations between the two are in [interoperability](https://github.com/cathode-ray-tube/moq-secure/blob/main/interoperability/README.md).

## Tests 

Run **rust** tests, from monorepo root (script generates test vectors first, populating `frames.json` file in the `test-vectors` folder):

```bash
npm run test-rust
```

Run **javascript** tests, from monorepo root (script generates test vectors first, populating `frames.json` file in the `test-vectors` folder):

```bash
npm test
```

## License

This project is dual-licensed: MIT OR Apache-2.0, choose either. See LICENSE-MIT and LICENSE-APACHE-2.0 in the repository root.


