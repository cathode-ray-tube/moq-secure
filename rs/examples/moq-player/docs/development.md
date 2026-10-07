# Development

## Build and run

```bash
cargo build
cargo run
```

Run the optimized build with:

```bash
cargo run --release
```

## Runtime prerequisites

The executable depends on system GStreamer plugins in addition to Rust crates. Before debugging Rust code, check that the runtime can create the required elements:

```bash
gst-inspect-1.0 moqsrc
gst-inspect-1.0 gtk4paintablesink
gst-inspect-1.0 pipewiresink
```

If `gst-inspect-1.0` succeeds in one environment but the application cannot find an element, check that both are using the same GStreamer installation, plugin paths, architecture, and registry.

## Debugging

The application logs dynamic pad caps, pad-link results, bus warnings, and bus errors to standard output and standard error.

Enable GStreamer diagnostics with:

```bash
GST_DEBUG=2 cargo run
```

To focus on a specific element, use a category filter:

```bash
GST_DEBUG=moqsrc:6,decodebin3:4 cargo run
```

## Useful code areas

- `Player::new`: constructs elements and links the fixed parts of the pipeline.
- `source.connect_pad_added`: routes incoming MoQ audio/video pads to parsers.
- Decoder `connect_pad_added` handlers: route decoded audio/video pads to their queues.
- `layout_tile_count`: defines how many streams each layout uses.
- `tile_position`: maps a tile index to GTK grid coordinates and spans.
- `build_ui`: creates the window, settings panel, tile grid, and controls.
- `main`: initializes GStreamer and starts the GTK application.

## Before submitting changes

- Run `cargo fmt`.
- Run `cargo check`.
- Run `cargo clippy` and address relevant warnings.
- Test with the required GStreamer plugins installed.
- Test at least one audio/video broadcast and each layout.
- Check that applying new settings stops the previous pipelines.
- Update the README and relevant docs when runtime requirements, supported formats, or controls change.

## Known documentation/code alignment items

Before release, confirm these items against the actual project configuration:

- The minimum supported GStreamer version.
- The exact MoQ plugin installation method and supported plugin builds.
- Whether `pipewiresink` is available on every platform listed as supported.
- The package name, Cargo installation instructions, and project license.
- Which codecs and broadcast formats the current MoQ plugin supplies.
