![moq-secure logo](./assets/moq-secure-logo-256.png)

[![JavaScript Tests](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-js.yml/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-js.yml)
[![Rust Tests](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-rust.yml/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-rust.yml)
[![CodeQL](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/github-code-scanning/codeql)

# MOQ-Secure Encryption & Signing

A fixed format for end-to-end encrypting **media payloads** carried by **Media Over QUIC (MOQ)** using **AEAD encryption (ChaCha20-Poly1305)** with an **optional Ed25519 signature**.

> **Payload-Only Encryption:** MOQ is a content-agnostic transport format. MOQ-Secure encrypts only the frame’s media payload bytes. Transport framing and routing remain unchanged.

## Purpose

People increasingly want to protect their communications from pervasive monitoring and mass surveillance. At the same time, audiences need confidence that media is genuine: in an era of deepfakes, you often can’t tell whether a video or audio clip truly came from the person it claims to be.

MOQ-Secure is designed to provide:
- **Privacy** for the media payload - so content can’t be inspected in transit
- **Integrity** - so tampering is detected
- **Authenticity** - so frames can be verified as coming from a broadcaster
- **Flexibility** - balancing security and performance

## Quick Start (moq-secure-chat-cli):

This demonstrates moq-secure end-to-end encryption and signing of text chat messages in the terminal.

### Prerequisites

Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Install **moq-relay** (the server) from the moq repo, full instructions [here](https://github.com/moq-dev/moq/tree/main/rs/moq-relay).

### Run

Run **moq-relay** with this config file, [localhost.toml](https://github.com/moq-dev/moq/blob/main/demo/relay/localhost.toml):

```bash
wget https://raw.githubusercontent.com/moq-dev/moq/refs/heads/main/demo/relay/localhost.toml
moq-relay localhost.toml
```

In a **2nd terminal**, clone this repo:

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
./moq-secure-chat-cli --relay https://localhost:4443/chat --tls-disable-verify publish
```
In a **3rd terminal**, paste the displayed command to run the binary in subscriber mode and receive sent messages.

### Troubleshooting

Run binary with `-h` or `--help` flag to list available args and usage:

```bash
./moq-secure-chat-cli --help
```
## Specification

The complete field layout, byte concatenation rules, nonce/AAD/digest definitions, and receiver processing order are in [specification](https://github.com/cathode-ray-tube/moq-secure/blob/main/specification/README.md).

Quick reference [format](https://github.com/cathode-ray-tube/moq-secure/blob/main/frame_format.txt).

## Interoperability

Only encrypts the payload so it should work with any MOQ implementation (moq-lite, IETF implementations, etc.).

While aimed at MOQ, with some additional wiring it could encrypt and sign data sent via other transports (such as WebSockets).

This repo contains implementations in [rust](https://github.com/cathode-ray-tube/moq-secure/tree/main/rs) and [javascript](https://github.com/cathode-ray-tube/moq-secure/tree/main/js). Compatability considerations between the two are in [interoperability](https://github.com/cathode-ray-tube/moq-secure/blob/main/interoperability/README.md).

## Tests 

**Test vectors** alone can be generated, from monorepo root (use commands further below to generate **and** run tests):

```bash
npm install
npm run vectors:generate
```

This will populate the `frames.json` file in the `test-vectors` directory.

Run **rust** tests, from monorepo root (script automatically generates test vectors first):

```bash
npm run test-rust
```

Run **javascript** tests, from monorepo root (script automatically generates test vectors first):

```bash
npm test
```

## License

This project is dual-licensed: MIT OR Apache-2.0, choose either. See LICENSE-MIT and LICENSE-APACHE-2.0 in the repository root.


