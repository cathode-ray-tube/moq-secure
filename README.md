![moq-secure logo](./assets/moq-secure-logo-256.png)

# MOQ-Secure Media Encryption & Signing

A fixed wire format for end-to-end encrypting **media payloads** carried by **Media Over QUIC (MOQ)** using AEAD encryption (ChaCha20-Poly1305) with an **optional Ed25519 signature**.

> **Payload-Only Encryption:** MOQ is a content-agnostic transport format. MOQ-Secure encrypts only the frame’s media payload bytes. Transport framing and routing remain unchanged.

## Why this exists

People increasingly want to protect their communications from pervasive monitoring and mass surveillance. At the same time, audiences need confidence that media is genuine: in an era of deepfakes, you often can’t tell whether a video or audio clip truly came from the person it claims to be.

MOQ-Secure is designed to provide:
- **Privacy** for the media payload - so content can’t be inspected in transit
- **Integrity** - so tampering is detected
- **Authenticity** - so frames can be verified as coming from a broadcaster
- **Flexibility** - balancing security and performance

## Quick Start (moq-secure-chat-cli):

This demonstrates moq-secure end-to-end encryption and signing of text chat messages in the terminal.

First, install and run a moq-relay from the moq repo, instructions [here](https://github.com/moq-dev/moq/tree/main/rs/moq-relay).

Clone:

```bash
git clone https://github.com/cathode-ray-tube/moq-secure.git
```

Build:

```bash
cd moq-secure

cargo build -p moq-secure-chat-cli
```

Run:

```bash
cd target/debug

./moq-secure-chat-cli --relay https://localhost:4443/chat --broadcast chat --tls-disable-verify publish
```
In a second terminal, paste the displayed command to run the binary in subscriber mode and receive sent messages.

Run binary with `-h` or `--help` flag to list available args.

## Wire format details

The complete on-the-wire field layout, byte concatenation rules, nonce/AAD/digest definitions, and receiver processing order live deeper in the repo for those who want the full technical [specification](https://github.com/cathode-ray-tube/moq-secure/blob/main/spec/README.md)

## Interop

Only encrypts the payload so it should work with any MOQ implementation (e.g., moq-lite, IETF implementations, etc.).

While aimed at MOQ, with some additional wiring it could encrypt any data sent via other transports (such as WebSockets).

## License

This project is dual-licensed: MIT OR Apache-2.0, choose either. See LICENSE-MIT and LICENSE-APACHE-2.0 in the repository root.


