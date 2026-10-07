# GStreamer pipeline

Each tile gets an independent GStreamer pipeline.

## Video branch

```text
moqsrc → h264parse → queue → decodebin3
                              └─ video pad → queue → videoconvert → gtk4paintablesink
```

`moqsrc` receives the configured URL and broadcast. Its dynamic pads are classified from their caps. The first enabled video pad is linked to `h264parse`.

The parser output enters a queue and then `decodebin3`. When the decoder exposes a video pad, that pad is linked to the decoded-video queue and the GTK sink branch.

The `gtk4paintablesink` exposes a `GdkPaintable`; the UI assigns it to the tile's `gtk::Picture`.

## Audio branch

```text
moqsrc → aacparse → decodebin3
                     └─ audio pad → queue → audioconvert → audioresample
                                      → capsfilter → volume → pipewiresink
```

The caps filter requests:

- Format: `S16LE`
- Layout: `interleaved`
- Rate: `48,000 Hz`
- Channels: 2

The volume is set to `0.7`. The sink has synchronization enabled and uses the timestamp offset configured by `AUDIO_TS_OFFSET_NS`.

## Queue settings

The source video and audio queues have a 500 ms time limit. The decoded-video queue has a 250 ms time limit. Buffer and byte limits are disabled, and the queues are configured as non-leaky.

These values are part of the current implementation, not user-facing settings.

## Inspecting the pipeline

Check that the required elements are installed:

```bash
gst-inspect-1.0 moqsrc
gst-inspect-1.0 h264parse
gst-inspect-1.0 aacparse
gst-inspect-1.0 decodebin3
gst-inspect-1.0 gtk4paintablesink
gst-inspect-1.0 pipewiresink
```

For runtime diagnostics, increase GStreamer logging:

```bash
GST_DEBUG=2 cargo run
```

For more detailed logs, raise the level selectively, for example:

```bash
GST_DEBUG=moqsrc:6,decodebin3:4 cargo run
```
