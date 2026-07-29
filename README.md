![moq-secure logo](./assets/moq-secure-logo-256.png)

# MOQ-Secure Media Frame Encryption

A fixed wire format for end-to-end encrypting **media payloads** carried by **Media Over QUIC (MOQ)** using AEAD encryption (ChaCha20-Poly1305) with an **optional Ed25519 signature**.

> **Payload-Only Encryption:** MOQ is a payload-agnostic transport format — this spec encrypts only the frame’s media payload bytes. Transport framing and routing remain unchanged.

## Why this exists

- **Confidentiality + integrity**: ChaCha20-Poly1305 (AEAD)
- **Optional authenticity**: Ed25519 signatures on selected frames
- **Fixed frame layout**:
  - Always includes a **64-byte `sigSlot`**
  - When unsigned, `sigSlot` is **all zeros**
- **Designed for lossy delivery & late join**:
  - A per-stream `ctr` enables safe nonce derivation (random start, monotonic increment)
  - Signed frames refresh a bounded “lease” for admitting unsigned frames

## Frame overview

Each encrypted frame contains:

1. **Unencrypted header (parse first)**  
   Fields include:
   - `magic` (constant `"MOQS"`)
   - `version`
   - `keyId` (supports key rotation)
   - `ctr` (uint64, random start then incrementing)
   - `nSigned` (lease/signing parameter; `0` disables Ed25519 entirely)
   - `sigFlag` (0 = unsigned, 1 = signed)
   - `sigSlot` (64 bytes; signature if signed, else all zeros)

2. **Encrypted payload (AEAD)**  
   - `ciphertext` (same length as plaintext)
   - `aeadTag` (16 bytes)
   - Encryption authenticates/binds the header fields via AEAD **AAD**, without including `sigSlot` in the AAD.

## Cryptographic primitives

- **Confidentiality & integrity**: `ChaCha20-Poly1305`
- **Nonce derivation**: derived from `(keyId, ctr)` to prevent nonce reuse
- **Optional signatures**: `Ed25519` over a `SHA-256` digest that excludes `sigSlot` bytes from the hashed structure

## Signing

Signature verification is **optional** and **selective** for performance: even when signature support is enabled, only a **subset** of frames carry an Ed25519 signature (`sigFlag = 1`). The remaining frames are sent unsigned (`sigFlag = 0`) with `sigSlot` set to 64 zero bytes.

The frequency of signing is set by the broadcaster/sender.

When signature support is disabled (`nSigned == 0`), all frames are unsigned (`sigFlag = 0`) and `sigSlot` must be all zeros; receivers skip signature verification entirely.

When signatures are enabled (`nSigned > 0`), signed frames are verified and act as periodic refresh points that allow a bounded amount of unsigned frames to be accepted.

## Wire format details

The complete on-the-wire field layout, byte concatenation rules, nonce/AAD/digest definitions, and receiver processing order live deeper in the repo for those who want the full technical [specification](https://github.com/cathode-ray-tube/moq-secure/blob/main/moq-secure/spec/README.md)

## Usage

This format is intended for implementations that want:
- MOQ’s payload-agnostic transport benefits, while
- securing media payload confidentiality and integrity on a per-frame basis,
- optionally adding authenticity for a subset of frames using a broadcaster key.

## Interop

Only encrypts the payload so should work with any MOQ implementation (moq-lite, IETF, etc).

## License

This project is dual-licensed: MIT OR Apache-2.0, choose either. See LICENSE-MIT and LICENSE-APACHE-2.0 in the repository root.
