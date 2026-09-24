# moq-player

![moq-player-screenshot](https://raw.githubusercontent.com/cathode-ray-tube/moq-secure/main/assets/moq-player.jpg)

A native desktop player for MoQ streams using Rust, GTK4, and GStreamer.

MOQ-Secure is not yet integrated.

This forms the basis of a MoQ media player (including encryption/signing).  I will include this in my [moq-tv](https://github.com/cathode-ray-tube/moq-tv) repo (mainly targeting Smart TVs at the moment).

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

## Install

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
