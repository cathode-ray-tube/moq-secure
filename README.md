![moq-secure logo](./assets/moq-secure-logo-256.png)

[![JavaScript Tests](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-js.yml/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-js.yml)
[![Rust Tests](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-rust.yml/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-rust.yml)
[![CodeQL](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/github-code-scanning/codeql)
![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)
[![crates.io version](https://img.shields.io/crates/v/moq-secure.svg)](https://crates.io/crates/moq-secure)
[![Downloads](https://img.shields.io/crates/d/moq-secure.svg)](https://crates.io/crates/moq-secure)
[![docs.rs](https://img.shields.io/docsrs/moq-secure)](https://docs.rs/moq-secure)

# MoQ-Secure Encryption & Signing

A fixed format for end-to-end secure **media payloads** carried by **Media over QUIC (MoQ)**.

Encryption algorithms:
- **ChaCha20-Poly1305**
- **AES-256-GCM**

Signature algorithm:
- **Ed25519**

> **Payload-Only Encryption:** MoQ is a content-agnostic transport format. MoQ-Secure encrypts only the frame’s media payload bytes. Transport framing and routing remain unchanged. **Metadata such as broadcast name, media codec and resolution remains unencrypted and visible to the relay.**

## Purpose

People increasingly want to protect their communications from pervasive monitoring and mass surveillance. At the same time, audiences need confidence that media is genuine: in an era of deepfakes, you often can’t tell whether a video or audio clip truly came from the person it claims to be.

MoQ-Secure is designed to provide:
- **Privacy** for the media payload - so content can’t be inspected in transit
- **Integrity** - so tampering is detected
- **Authenticity** - so frames can be verified as coming from a particular publisher
- **Flexibility** - balancing security and performance

Publishers are able to use **any** public MoQ CDN, with the hosting provider unable to see the content. Consumers can verify the publisher of the content, wherever they receive it from.

## Quick Start (`moq-secure-chat-cli`)

A terminal chat demo using MoQ with end-to-end encryption and Ed25519 message signing via [`moq-secure-chat`](https://github.com/cathode-ray-tube/moq-secure/tree/main/rs/examples/moq-secure-chat).

Install Rust and [moq-relay](https://github.com/moq-dev/moq/tree/main/rs/moq-relay), then start a local relay using the example configuration:

```bash
wget https://raw.githubusercontent.com/moq-dev/moq/refs/heads/main/demo/relay/localhost.toml
moq-relay localhost.toml
```

In another terminal, install and start the publisher:

```bash
cargo install moq-secure-chat-cli

moq-secure-chat-cli \
  --relay https://localhost:4443/chat \
  --tls-disable-verify \
  publish
```

The publisher generates the broadcast name and cryptographic keys, then prints a complete subscriber command. Copy that command into a third terminal and run it.

Type messages in the publisher terminal to send encrypted, signed chat messages.

The default encryption algorithm is ChaCha20-Poly1305. AES-256-GCM is also supported with:

```bash
--encryption aes-256-gcm
```

This is a demonstration application. For production use, harden key management, key distribution, authentication, and TLS configuration.

See the full [README.md](https://github.com/cathode-ray-tube/moq-secure/blob/main/rs/examples/moq-secure-chat-cli/README.md) for complete usage, configuration options, troubleshooting, and security notes.
## Quick Start (`moq-player`)

![moq-player screenshot](https://raw.githubusercontent.com/cathode-ray-tube/moq-secure/main/assets/moq-player.jpg)

[![crates.io version](https://img.shields.io/crates/v/moq-player.svg)](https://crates.io/crates/moq-player)
[![Downloads](https://img.shields.io/crates/d/moq-player.svg)](https://crates.io/crates/moq-player)

A native desktop player for MoQ audio/video streams using Rust, GTK4, and GStreamer.

> **MoQ-Secure is not integrated.** This version does not encrypt, decrypt, or verify media using MoQ-Secure.

### What it does

The player subscribes to MoQ streams with GStreamer's `moqsrc`, decodes H.264 video and AAC audio, and displays video in a GTK4 window. Open **Settings** to choose a layout and configure a URL and broadcast for each tile.

Available layouts:

- One full-size tile
- Three tiles side by side
- Four tiles in a 2×2 grid
- One large tile with two smaller stacked tiles

Each tile runs its own playback pipeline. **Apply & Play** replaces the current players and starts the configured streams.

### Supported platforms

- Ubuntu and Debian
- Fedora, RHEL, Rocky Linux, and AlmaLinux
- macOS on Apple Silicon: **not confirmed with the current audio pipeline**
- Windows is not currently supported

The audio pipeline uses `pipewiresink`. Confirm that this sink and a working PipeWire setup are available on your target system. macOS may require an alternate GStreamer audio sink and a corresponding code change.

### Requirements

- Rust
- GTK4
- GStreamer development libraries and runtime plugins
- The MoQ GStreamer plugin, providing `moqsrc`
- The GStreamer GTK4 video sink, providing `gtk4paintablesink`
- GStreamer elements used by the pipeline:
  - `h264parse`
  - `aacparse`
  - `decodebin3`
  - `videoconvert`
  - `audioconvert`
  - `audioresample`
  - `pipewiresink`

The code currently requests GStreamer **1.24 or newer**. Install the GStreamer packages and plugins available for your distribution.

### Install dependencies

#### Ubuntu or Debian

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

Install the MoQ GStreamer plugin using the repository instructions for your distribution. For the MoQ APT repository:

```bash
curl -fsSL https://apt.moq.dev/moq-keyring.gpg \
  | sudo tee /usr/share/keyrings/moq-keyring.gpg >/dev/null

echo "deb [signed-by=/usr/share/keyrings/moq-keyring.gpg] https://apt.moq.dev stable main" \
  | sudo tee /etc/apt/sources.list.d/moq.list >/dev/null

sudo apt update
sudo apt install -y gstreamer1.0-moq
```

#### Fedora, RHEL, Rocky Linux, or AlmaLinux

On RHEL, Rocky Linux, and AlmaLinux, ensure the standard BaseOS and AppStream repositories are enabled.

```bash
sudo dnf install -y \
  dnf-plugins-core \
  gcc \
  gcc-c++ \
  make \
  curl \
  pkgconf-pkg-config \
  openssl-devel \
  gtk4-devel \
  gstreamer1-devel \
  gstreamer1-plugins-base-devel \
  gstreamer1-plugins-base \
  gstreamer1-plugins-good \
  gstreamer1-plugins-bad-free \
  gstreamer1-plugins-bad-free-extras \
  gstreamer1-libav \
  gstreamer1-plugins-base-tools
```

Install the package that provides `gtk4paintablesink` if it is not included above. Package names vary by distribution; search for available packages with:

```bash
dnf search gstreamer gtk4
```

Install the MoQ GStreamer plugin using the repository instructions for your distribution. If the MoQ RPM repository is available:

```bash
sudo dnf config-manager --add-repo https://rpm.moq.dev/moq.repo
sudo dnf install -y gstreamer1-moq
```

#### macOS

The current code uses `pipewiresink` for audio. The instructions below describe installing GTK4, GStreamer, and the MoQ plugin, but do not by themselves provide a macOS-compatible audio sink. macOS playback may require changing the audio sink in the application.

For an Apple Silicon setup using Homebrew:

```bash
brew install gtk4 gstreamer
```

Install or build the GStreamer Rust plugin that provides `gtk4paintablesink`. If available for your setup:

```bash
brew install gst-plugins-rs
```

If it is not available, follow the GStreamer plugin build instructions for your chosen GStreamer installation. Keep the GTK and GStreamer installations consistent; avoid mixing Homebrew libraries with the official GStreamer framework unless you configure and test the paths.

Download the Apple Silicon `moq-gst` archive from the MoQ project's [GitHub releases](https://github.com/moq-dev/moq/releases), then install its plugin:

```bash
tar -xzf moq-gst-*.tar.gz
cd moq-gst-*

mkdir -p "$HOME/Library/Application Support/GStreamer/1.0/plugins"

cp lib/gstreamer-1.0/libgstmoq.dylib \
  "$HOME/Library/Application Support/GStreamer/1.0/plugins/"
```

If using the official GStreamer installer, add its tools to `PATH`:

```bash
export PATH="/Library/Frameworks/GStreamer.framework/Versions/1.0/bin:$PATH"
```

For a Homebrew installation, libraries are commonly located under `/opt/homebrew/lib`. If needed:

```bash
export DYLD_FALLBACK_LIBRARY_PATH="/opt/homebrew/lib:$DYLD_FALLBACK_LIBRARY_PATH"
```

### Verify GStreamer plugins

Check that GStreamer can find the main elements:

```bash
gst-inspect-1.0 moqsrc
gst-inspect-1.0 gtk4paintablesink
gst-inspect-1.0 h264parse
gst-inspect-1.0 aacparse
gst-inspect-1.0 decodebin3
gst-inspect-1.0 pipewiresink
```

All elements must be available to the GStreamer installation used by the application. The code uses `moqsrc`; `moqsink` is not required for playback.

### Install Rust

If Rust is not installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Check the installation:

```bash
rustc --version
cargo --version
```

### Install and run

```bash
cargo install moq-player
moq-player
```

Alternatively, build and run from the project source:

```bash
cargo run --release
```

The application starts with the default stream:

```text
URL:       https://cdn.moq.dev/demo
Broadcast: bbb.hang
```

### Use the player

1. Open **Settings**.
2. Choose a layout.
3. Enter a MoQ URL and broadcast name for each tile.
4. Select **Apply & Play**.
5. Use **Play** and **Stop** to control the active players.

Changing layouts rebuilds the settings panel and changes how many stream inputs are shown. Applying settings stops and replaces the current player pipelines.

### Troubleshooting

#### A GStreamer element cannot be created

Check that the element is installed and visible:

```bash
gst-inspect-1.0 ELEMENT_NAME
```

For example:

```bash
gst-inspect-1.0 moqsrc
gst-inspect-1.0 gtk4paintablesink
gst-inspect-1.0 pipewiresink
```

Make sure the application and `gst-inspect-1.0` use the same GStreamer installation and plugin paths.

#### The MoQ plugin is not detected on macOS

Set the plugin path and test discovery:

```bash
export GST_PLUGIN_PATH="$HOME/Library/Application Support/GStreamer/1.0/plugins"
gst-inspect-1.0 moqsrc
```

#### Clear the GStreamer plugin cache

Linux:

```bash
rm -f ~/.cache/gstreamer-1.0/registry-*.bin
```

macOS:

```bash
rm -f "$HOME/Library/Caches/GStreamer/1.0/registry-"*.bin
```

Then retry the relevant `gst-inspect-1.0` commands.

#### A tile stays blank or playback fails

Run the application from a terminal and check its output for GStreamer errors and pad-link messages. Confirm that the broadcast provides supported H.264 video or AAC audio and that the required plugins are installed.

#### Audio does not play

The current pipeline sends audio to `pipewiresink`. Check that it is installed and that PipeWire is running:

```bash
gst-inspect-1.0 pipewiresink
```

### Windows

Windows is not currently supported.

The MoQ plugin releases listed by the project provide binaries for:

```text
x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu
aarch64-apple-darwin
```

## Specification

The complete field layout, byte concatenation rules, nonce/AAD/digest definitions, and receiver processing order are in [specification](https://github.com/cathode-ray-tube/moq-secure/blob/main/specification/README.md).

Quick reference [format](https://github.com/cathode-ray-tube/moq-secure/blob/main/frame_format.txt).

## Interoperability

Only encrypts the payload so it should work with any MoQ implementation (moq-lite, IETF implementations, etc.).

While aimed at MoQ, with some additional wiring it could encrypt and sign data sent via other transports (such as WebSockets, WebRTC Data Channels, etc).

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


