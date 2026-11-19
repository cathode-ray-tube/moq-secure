![moq-secure-logo-and-badges](./assets/moq-secure-new.jpeg)

[![JavaScript Tests](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-js.yml/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-js.yml)
[![Rust Tests](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-rust.yml/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/test-rust.yml)
[![CodeQL](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/cathode-ray-tube/moq-secure/actions/workflows/github-code-scanning/codeql)
![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)
[![crates.io version](https://img.shields.io/crates/v/moq-secure.svg)](https://crates.io/crates/moq-secure)
[![npm](https://img.shields.io/npm/v/moq-secure)](https://www.npmjs.com/package/@cathode-ray-tube/moq-secure)
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

## Quick Start

To try the examples, you’ll need a MoQ relay URL and a broadcast name.

1. Open the [`@cathode-ray-tube/moq-secure-publish` README](https://www.npmjs.com/package/@cathode-ray-tube/moq-secure-publish) and follow its example to share video from your screen.
2. Open the [`@cathode-ray-tube/moq-secure-watch` README](https://www.npmjs.com/package/@cathode-ray-tube/moq-secure-watch) and follow its example to watch the broadcast.
3. In both examples, enter the same relay URL, broadcast name, and encryption key settings.

The README examples use a test key only. Don’t use it to protect a real broadcast.

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


