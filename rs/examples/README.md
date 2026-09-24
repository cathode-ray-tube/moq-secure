# Rust Examples

This directory contains Rust applications demonstrating media and chat functionality.

- [moq-player](https://github.com/cathode-ray-tube/moq-secure/tree/main/rs/examples/moq-player) — A native desktop player for MoQ streams using Rust, GTK4 and GStreamer.. It **does not** currently integrate with `moq-secure`.
- [moq-secure-chat](https://github.com/cathode-ray-tube/moq-secure/tree/main/rs/examples/moq-secure-chat) — Chat functionality wrapper built on top of `moq-secure`.
- [moq-secure-chat-cli](https://github.com/cathode-ray-tube/moq-secure/tree/main/rs/examples/moq-secure-chat-cli) — Command-line chat application using `moq-secure-chat`.

Build all examples from the repository root:

```bash
cargo build --workspace
```

There are further examples in my fork of the moq repo, [here](https://github.com/cathode-ray-tube/moq/blob/dev/rs/moq-mux/examples/README.md).
