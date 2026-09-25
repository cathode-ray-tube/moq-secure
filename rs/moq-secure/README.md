# MOQ-Secure Encryption & Signing

![moq-secure frame layout](https://raw.githubusercontent.com/cathode-ray-tube/moq-secure/blob/main/assets/moq-secure-layout.jpeg)

[![crates.io version](https://img.shields.io/crates/v/moq-secure.svg)](https://crates.io/crates/moq-secure)
[![Downloads](https://img.shields.io/crates/d/moq-secure.svg)](https://crates.io/crates/moq-secure)
[![docs.rs](https://img.shields.io/docsrs/moq-secure)](https://docs.rs/moq-secure)

A fixed format for end-to-end encrypting **media payloads** carried by **Media Over QUIC (MOQ)** using **AEAD encryption (ChaCha20-Poly1305)** with an **optional Ed25519 signature**.

> **Payload-Only Encryption:** MOQ is a content-agnostic transport format. MOQ-Secure encrypts only the frame’s media payload bytes. Transport framing and routing remain unchanged. **Metadata such as broadcast name, media codec and resolution remains unencrypted and visible to the relay.**

## Purpose

People increasingly want to protect their communications from pervasive monitoring and mass surveillance. At the same time, audiences need confidence that media is genuine: in an era of deepfakes, you often can’t tell whether a video or audio clip truly came from the person it claims to be.

MOQ-Secure is designed to provide:
- **Privacy** for the media payload - so content can’t be inspected in transit
- **Integrity** - so tampering is detected
- **Authenticity** - so frames can be verified as coming from a particular publisher
- **Flexibility** - balancing security and performance

Publishers are able to use **any** public MOQ CDN, with the hosting provider unable to see the content. Consumers can verify the publisher of the content, wherever they receive it from.
