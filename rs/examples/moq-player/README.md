# moq-player

A native desktop player for MoQ audio/video streams, built with Rust, GTK4, and GStreamer.

The application subscribes to a MoQ broadcast through GStreamer's `moqsrc` element, decodes H.264 video and AAC audio, and renders them in a GTK4 window. Use the Settings panel to choose a layout and configure the URL and broadcast for each tile.

> **Status:** This player currently plays MoQ streams. MoQ-Secure encryption and signature verification are not integrated.

![moq-player screenshot](https://raw.githubusercontent.com/cathode-ray-tube/moq-secure/main/assets/moq-player.jpg)

## Features

- Subscribe to a MoQ stream using a URL and broadcast name.
- Play video and audio through GStreamer.
- Display one, three, or four stream tiles.
- Choose from four layouts:
  - One full-size tile
  - Three side-by-side tiles
  - A 2×2 grid
  - One large tile with two smaller stacked tiles
- Configure each tile's MoQ URL and broadcast in the Settings panel.
- Start and stop all configured players from the main window.

The layout buttons determine the number of active tiles. The three-tile layouts use three streams; the 2×2 layout uses four. Each tile currently uses the same playback pipeline and enables both audio and video.

## Requirements

- Rust toolchain
- GTK4 development libraries
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
  - `volume`
  - `pipewiresink`

The audio output is currently configured to use `pipewiresink`, so a working PipeWire audio setup is needed for audio playback.

## Install system dependencies

Package names and plugin availability vary between operating-system releases. Install the GTK4 and GStreamer development packages, GStreamer runtime plugins, and the MoQ plugin appropriate for your system.

### Ubuntu or Debian

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

Install the MoQ GStreamer plugin using the current instructions from the plugin provider. For example, if the MoQ APT repository is available for your distribution:

```bash
curl -fsSL https://apt.moq.dev/moq-keyring.gpg \
  | sudo tee /usr/share/keyrings/moq-keyring.gpg >/dev/null

echo "deb [signed-by=/usr/share/keyrings/moq-keyring.gpg] https://apt.moq.dev stable main" \
  | sudo tee /etc/apt/sources.list.d/moq.list >/dev/null

sudo apt update
sudo apt install -y gstreamer1.0-moq
```

### Fedora, RHEL, Rocky Linux, or AlmaLinux

Install the GTK4, GStreamer development, and runtime packages available for your distribution. For example:

```bash
sudo dnf install -y \
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

Install the package that provides `gtk4paintablesink` if it is not included in the packages above. Package names can differ across Fedora and Enterprise Linux releases.

Install the MoQ plugin using the instructions for your distribution. If the MoQ RPM repository is available:

```bash
sudo dnf config-manager --add-repo https://rpm.moq.dev/moq.repo
sudo dnf install -y gstreamer1-moq
```

### macOS

The code uses `pipewiresink` for audio output. PipeWire is not a standard macOS audio backend, so macOS playback will require a compatible audio sink or a code/configuration change.

For GTK4 and GStreamer, install a consistent set of development and runtime packages. For example:

```bash
brew install gtk4 gstreamer
```

Install the GStreamer plugins that provide `gtk4paintablesink`, the parsers, and the decoders used by the pipeline. Install the MoQ plugin build that matches your GStreamer installation and Apple Silicon architecture.

Do not mix Homebrew GStreamer libraries with the official GStreamer framework unless you configure and test the library and plugin paths carefully.

## Verify GStreamer plugins

Check that GStreamer can find the elements required by the application:

```bash
gst-inspect-1.0 moq
gst-inspect-1.0 gtk4paintablesink
gst-inspect-1.0 h264parse
gst-inspect-1.0 aacparse
gst-inspect-1.0 decodebin3
gst-inspect-1.0 pipewiresink
```

The MoQ plugin must expose `moqsrc`. The application also constructs the other listed elements at runtime; a missing element prevents player creation.

To check GStreamer plugin discovery in more detail:

```bash
GST_DEBUG=2 gst-inspect-1.0 moq
```

## Install Rust

If Rust is not installed, install it with rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Verify the toolchain:

```bash
rustc --version
cargo --version
```

## Build and run from source

From the project directory:

```bash
cargo build --release
cargo run --release
```

The application starts with the default URL and broadcast configured in the source:

```text
https://cdn.moq.dev/demo
bbb.hang
```

It starts the initial player automatically. Open **Settings** to select a layout and configure stream inputs. Select **Apply & Play** to recreate the players using those settings.

The project is published as a Cargo package. It can also be installed with:

```bash
cargo install moq-player
moq-player
```

## Using the player

1. Launch the application. The default stream starts automatically.
2. Select **Settings** to open the side panel.
3. Choose a layout using the layout buttons.
4. Enter a MoQ URL and broadcast name for each tile.
5. Select **Apply & Play** to create and start the configured players.
6. Use **Play** or **Stop** to control all active players.
7. Select **Quit** or close the window to exit.

Changing a layout rebuilds the settings panel and the number of stream inputs. Applying settings stops the old pipelines, replaces them, and starts the new ones.

## Troubleshooting

### `Could not create element ...`

A required GStreamer element is missing or is not visible to the GStreamer runtime used by the application. Check it with:

```bash
gst-inspect-1.0 ELEMENT_NAME
```

For example:

```bash
gst-inspect-1.0 moqsrc
gst-inspect-1.0 gtk4paintablesink
gst-inspect-1.0 pipewiresink
```

Make sure `gst-inspect-1.0` and the application use the same GStreamer installation and plugin paths.

### MoQ plugin is not detected

Check the plugin installation and search path:

```bash
gst-inspect-1.0 moq
```

On macOS, set `GST_PLUGIN_PATH` to the directory containing the MoQ plugin if needed. Clear the GStreamer registry cache after installing or replacing plugins.

Linux:

```bash
rm -f ~/.cache/gstreamer-1.0/registry-*.bin
```

macOS:

```bash
rm -f "$HOME/Library/Caches/GStreamer/1.0/registry-"*.bin
```

### The window opens but a tile stays blank

Check the application’s terminal output for messages about MoQ pads, caps, and GStreamer errors. Confirm that the broadcast contains supported audio or video streams and that the required parser and decoder plugins are installed.

### Video works but audio does not

The current pipeline sends audio to `pipewiresink`. Confirm that PipeWire is running and that `pipewiresink` is available:

```bash
gst-inspect-1.0 pipewiresink
```

### Audio/video timing needs adjustment

The pipeline currently sets a fixed audio sink timestamp offset in the source code. This value may need tuning for a particular stream, sink, or system.

## Current limitations

- MoQ-Secure encryption and signature verification are not implemented.
- The input pipeline expects H.264 video and AAC audio.
- Audio output is hard-coded to `pipewiresink`.
- Each tile creates its own playback pipeline; configuring several tiles can use substantial CPU, GPU, network, and audio resources.
- The application links the first matching audio and video pads. Additional pads of the same media type are ignored.
- Stream configuration is entered in the UI and is not persisted between launches.
- Windows is not currently supported.

## Security and planned work

The current player receives and decodes stream media; it does not authenticate or decrypt media using MoQ-Secure. Do not treat the current application as providing end-to-end media authenticity or confidentiality beyond protections provided by the transport and deployment.

The intended future processing flow is:

```text
MoQ source → verify/decrypt media objects → parse/decode → render
```

Any secure-object integration should verify data before it reaches the decoder and define key distribution, failure handling, and user-visible trust status.

## Documentation

- [Architecture and code walkthrough](docs/architecture.md)
- [GStreamer pipeline](docs/pipeline.md)
- [Development notes](docs/development.md)

## License

Add the license for this project here, and include the corresponding license file(s) in the repository.

