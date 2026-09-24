![moq-secure logo](./assets/moq-secure-logo-256.png)

[![JavaScript Tests](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-js.yml/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-js.yml)
[![Rust Tests](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-rust.yml/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-rust.yml)
[![CodeQL](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/github-code-scanning/codeql)
![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)
[![crates.io version](https://img.shields.io/crates/v/moq-secure.svg)](https://crates.io/crates/moq-secure)
[![Downloads](https://img.shields.io/crates/d/moq-secure.svg)](https://crates.io/crates/moq-secure)
[![docs.rs](https://img.shields.io/docsrs/moq-secure)](https://docs.rs/moq-secure)

# MOQ-Secure Encryption & Signing

A fixed format for end-to-end secure **media payloads** carried by **Media Over QUIC (MOQ)**.

Encryption algorithms:
- **ChaCha20-Poly1305**
- AES-GCM 128 (*Coming Soon*)

Signature algorithm:
- **Ed25519**

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

[![crates.io version](https://img.shields.io/crates/v/moq-secure-chat-cli.svg)](https://crates.io/crates/moq-secure-chat-cli)
[![Downloads](https://img.shields.io/crates/d/moq-secure-chat-cli.svg)](https://crates.io/crates/moq-secure-chat-cli)

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

A native desktop player for MoQ streams using Rust, GTK4 and GStreamer.

MOQ-Secure is not yet integrated.

### Supported platforms

- Ubuntu and Debian
- Fedora, RHEL, Rocky Linux, and AlmaLinux
- macOS on Apple Silicon

Windows is not currently supported.

### Requirements

The application requires:

- Rust
- GTK4
- GStreamer 1.24 or newer
- The MoQ GStreamer plugin
- The GStreamer GTK4 video sink

The MoQ plugin provides:

- `moqsrc`
- `moqsink`

The GTK4 video sink provides:

- `gtk4paintablesink`

### Install dependencies

#### Ubuntu or Debian

Install the required packages:

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

Install the MoQ GStreamer plugin:

```bash
curl -fsSL https://apt.moq.dev/moq-keyring.gpg \
  | sudo tee /usr/share/keyrings/moq-keyring.gpg >/dev/null

echo "deb [signed-by=/usr/share/keyrings/moq-keyring.gpg] https://apt.moq.dev stable main" \
  | sudo tee /etc/apt/sources.list.d/moq.list >/dev/null

sudo apt update
sudo apt install -y gstreamer1.0-moq
```

#### Fedora, RHEL, Rocky Linux, or AlmaLinux

These distributions use `dnf`.

On RHEL, Rocky Linux, and AlmaLinux, ensure that the standard BaseOS and AppStream repositories are enabled.

Install the required packages:

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
  gstreamer1-plugins-base-tools \
  gstreamer1-plugins-rs
```

Some distributions use a different package name for the Rust-based GStreamer plugins. If `gstreamer1-plugins-rs` is unavailable, search for the available package:

```bash
dnf search gstreamer gtk4
```

Install the package that provides `gtk4paintablesink`.

Install the MoQ GStreamer plugin:

```bash
sudo dnf config-manager --add-repo https://rpm.moq.dev/moq.repo
sudo dnf install -y gstreamer1-moq
```

#### macOS

The prebuilt MoQ plugin currently supports Apple Silicon Macs.

Install Homebrew packages:

```bash
brew install gtk4 gstreamer
```

The GTK4 GStreamer sink is provided by the GStreamer Rust plugins. Install the package if it is available for your Homebrew setup:

```bash
brew install gst-plugins-rs
```

If Homebrew does not provide `gst-plugins-rs`, install the official GStreamer runtime and development packages from:

<https://gstreamer.freedesktop.org/download/>

Use one GStreamer installation consistently. Do not mix Homebrew GStreamer libraries with the official GStreamer framework unless you configure the library paths carefully.

Download the macOS Apple Silicon `moq-gst` tarball from the project's GitHub Releases page.

Extract and install the MoQ plugin:

```bash
tar -xzf moq-gst-*.tar.gz
cd moq-gst-*

mkdir -p "$HOME/Library/Application Support/GStreamer/1.0/plugins"

cp lib/gstreamer-1.0/libgstmoq.dylib \
  "$HOME/Library/Application Support/GStreamer/1.0/plugins/"
```

If GStreamer was installed with the official installer, set the command path:

```bash
export PATH="/Library/Frameworks/GStreamer.framework/Versions/1.0/bin:$PATH"
```

If GStreamer was installed with Homebrew, its libraries are normally located at:

```text
/opt/homebrew/lib
```

If necessary, set:

```bash
export DYLD_FALLBACK_LIBRARY_PATH="/opt/homebrew/lib:$DYLD_FALLBACK_LIBRARY_PATH"
```
### Verify the GStreamer installation

Same commands on every supported platform:

```bash
gst-inspect-1.0 moq
gst-inspect-1.0 gtk4paintablesink
```

Both commands must succeed.

The MoQ plugin should list:

```text
moqsrc
moqsink
```

The second command should display information about:

```text
gtk4paintablesink
```

If either command fails, `moq-player` will not run correctly. See `Troubleshooting` below.

### Install Rust

Install Rust if it is not already installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Load Rust into the current terminal:

```bash
source "$HOME/.cargo/env"
```

Verify the installation:

```bash
rustc --version
cargo --version
```

### Install moq-player

```bash
cargo install moq-player
```

### Run moq-player

```bash
moq-player
```

A window should open and begin playing the default `Big Buck Bunny` broadcast.

### Troubleshooting

#### macOS plugin path

If macOS cannot find the MoQ plugin, set the plugin path manually:

```bash
export GST_PLUGIN_PATH="$HOME/Library/Application Support/GStreamer/1.0/plugins"
gst-inspect-1.0 moq
```

#### Clear the GStreamer plugin cache

If a plugin was installed but is not detected, clear the plugin cache.

Linux:

```bash
rm -f ~/.cache/gstreamer-1.0/registry-*.bin
```

macOS:

```bash
rm -f "$HOME/Library/Caches/GStreamer/1.0/registry-"*.bin
```

Then retry:

```bash
gst-inspect-1.0 moq
gst-inspect-1.0 gtk4paintablesink
```

### Windows

Windows is not currently supported.

The current MoQ plugin releases provide binaries for:

```text
x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu
aarch64-apple-darwin
```

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


