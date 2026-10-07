# Architecture and code walkthrough

This document describes the implementation shown in the player source code.

## Application overview

The application has three main parts:

1. **`Player`** owns a GStreamer pipeline and its video sink.
2. **Pipeline construction** connects MoQ source pads to audio and video processing branches.
3. **GTK UI** manages players, layouts, stream-entry fields, and playback controls.

The UI is built on GTK's main thread. GStreamer provides decoded media to its sinks, while GTK displays the video sink's `GdkPaintable` in a `gtk::Picture`.

## `Player`

```rust
struct Player {
    pipeline: gst::Pipeline,
    video_sink: gst::Element,
}
```

`Player::new` constructs a pipeline for a single URL and broadcast. It creates the source, parsers, decoders, converters, and sinks, adds them to the pipeline, and links the fixed sections of each branch.

The source pads are dynamic: `moqsrc` creates them when stream tracks become available. The application listens for `pad-added`, checks the pad caps, and links the first enabled audio and video pads to their respective parsers.

The `enable_video` and `enable_audio` arguments control whether pads of those media types are linked. In the UI code shown, both are passed as `true`.

`play` changes the pipeline to `Playing`; `stop` changes it to `Null`. `Drop` calls `stop`, so dropping the last `Rc<Player>` also stops its pipeline.

## Pipeline

### Video

```text
moqsrc
  → h264parse
  → queue
  → decodebin3
  → queue
  → videoconvert
  → gtk4paintablesink
```

The parser is configured with `config-interval = 1`. Queues have time-based limits and are non-leaky. The decoded video queue is linked dynamically when `decodebin3` exposes a video pad.

The GTK sink exposes a `paintable` property. The UI periodically checks that paintable's intrinsic dimensions are available, then assigns it to a `gtk::Picture`.

### Audio

```text
moqsrc
  → aacparse
  → decodebin3
  → queue
  → audioconvert
  → audioresample
  → capsfilter
  → volume
  → pipewiresink
```

The caps filter requests interleaved, stereo, 48 kHz `S16LE` audio. The volume element is set to `0.7`. The sink is configured with synchronization enabled and a fixed timestamp offset.

The audio output element is currently specific to PipeWire. Supporting other systems may require selecting a platform-appropriate sink or exposing sink choice as configuration.

## Dynamic pad linking

For the source, the code:

1. Reads the pad's current caps, falling back to queried caps.
2. Examines the first caps structure.
3. Classifies caps beginning with `video/` or `audio/`.
4. Selects the matching parser's sink pad, if that media type is enabled.
5. Skips the pad if that sink is already linked.
6. Attempts to link the source pad to the parser.

Each decoder also emits `pad-added`. The callback checks whether the new pad is audio or video, then links it to the corresponding queue if that queue is not already linked.

This means the implementation handles one audio pad and one video pad per player. It does not select among multiple tracks, handle alternate audio languages, or route multiple video tracks through a single player.

## GTK interface

`build_ui` constructs the main window with:

- A horizontal content area.
- A `gtk::Grid` for video tiles.
- A slide-in settings panel.
- Settings, Play, Stop, and Quit buttons.

The settings panel is controlled by a `gtk::Revealer`. Selecting a layout updates the selected layout and rebuilds the panel, including the number of URL/broadcast input pairs.

### Layout mapping

| Layout index | Tile count | Arrangement |
|---|---:|---|
| `0` | 3 | Three tiles side by side |
| `1` | 4 | 2×2 grid |
| `2` | 3 | One large tile left, two stacked tiles right |
| `3` | 1 | One full-size tile |

`tile_position` returns each tile's grid coordinates and span. The 2×2 layout uses one grid cell per tile; the main-plus-two layout spans two columns and two rows for the main tile.

### Applying settings

When **Apply & Play** is selected, the callback:

1. Creates a new `Player` for each input pair.
2. Installs a GStreamer bus watch for each new pipeline.
3. Stops the old players.
4. Replaces the player list.
5. Clears the tile grid and creates one `gtk::Picture` per player.
6. Attaches each frame to the grid using the selected layout.
7. Starts each player.

If creation of any player fails, the callback logs the error and returns before replacing the current players.

The initial player is created separately in `main`, starts immediately, and is displayed in the initial one-tile layout.

## GStreamer bus messages

`install_bus_watch` logs GStreamer errors and warnings, including the originating element path and debug details when available. End-of-stream is also logged.

The watch currently logs messages but does not present an error dialog, stop sibling players, or update the tile UI. Those are possible improvements for a more user-facing error experience.

## Ownership and lifecycle

The GTK callbacks and timeout closures use `Rc` to share player and UI state on the main thread. The active player list is held in `Rc<RefCell<Vec<Rc<Player>>>>`.

Replacing the list drops old `Rc<Player>` references once they are no longer held elsewhere. `Player::drop` then sets the corresponding pipeline to `Null`.

Each picture has a local timeout that polls its player's paintable every 250 ms. It ends when a paintable with non-zero intrinsic dimensions is available.

## Current constraints and extension points

- **Track selection:** The first audio and first video pads are used. Add track selection or explicit pad routing if a broadcast has multiple tracks.
- **Media types:** The pipeline is built for H.264 and AAC. Other codecs need suitable parser/decoder paths.
- **Audio sink:** Replace or configure `pipewiresink` to support other operating systems and audio backends.
- **Error reporting:** Convert bus errors into UI state or notifications in addition to terminal logs.
- **Saved configuration:** Persist layout and per-tile stream inputs if settings should survive application restarts.
- **Secure media:** Insert verification and decryption before parsers/decoders, and define how keys are provisioned and how verification failures are handled.
- **Pipeline construction:** Factor branch creation and sink selection into configuration if more codecs or output devices are added.
