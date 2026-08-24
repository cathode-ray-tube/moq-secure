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

## Quick Start (Try linux-native-video)

From main repo root:
```bash
cargo build -p linux-native-video
```
When built:
```bash
cd target/debug
./linux-native-video
```

## Frame overview

Each frame contains an unencrypted header followed by a payload area and an optional signature trailer.

1. **Unencrypted header (parse first)**  
   Fields:
   - `magic` (constant `"MOQS"`)
   - `version`
   - `keyId` (supports key rotation)
   - `ctr` (uint64, random start then incrementing)
   - `nSigned` (lease/signing parameter; `0` disables Ed25519 entirely)
   - `sigFlag` (0 = unsigned, 1 = signed)
   - `encrypted` (normally 1; 0 when only signing is required / no AEAD encryption)
   - `padLen` (4 bytes, last in the header) — indicates the number of **padding bytes prepended to the plaintext before encryption** (padding may be present; if present, it is prepended to plaintext prior to encryption)

2. **Payload area (conditional on `encrypted`)**
   - If `encrypted = 1`:
     - `ciphertext` (same length as plaintext)
     - `aeadTag` (16 bytes)
   - If `encrypted = 0`:
     - `ciphertext` is replaced by **plaintext bytes directly**
     - `aeadTag` is omitted entirely (no AEAD is used)

   **AAD binding:** When AEAD is used, encryption authenticates/binds the unencrypted header fields via AEAD **AAD** (the signature is not in the AAD; see signing trailer below).

3. **Signature trailer (only when signed)**
   - If `sigFlag = 1`:
     - Append a 64-byte `ed25519Signature` **at the very end of the frame**, i.e. after:
       - `aeadTag` when `encrypted = 1`, or
       - the plaintext bytes when `encrypted = 0`.
   - If `sigFlag = 0`:
     - No signature bytes are appended.

### Padding and usable plaintext offset

When `encrypted = 1`, `padLen` lets receivers compute how much of the decrypted plaintext corresponds to usable media data.

Let:
- `plaintext` be the decrypted bytes (length equals ciphertext length)
- `padLen` be the padding length from the header (0 allowed)

Then, since padding is **prepended**:
- `usablePlaintext = plaintext[padLen : len(plaintext)]`

When `encrypted = 0`, padding (if any) is likewise treated as prepended to plaintext bytes before any signing-only processing.

### Frame layout (nested within MOQ frame payload):

![moq-secure frame layout](https://github.com/cathode-ray-tube/moq-secure/blob/main/assets/moq-secure-frame-layout.jpeg)

## Keys

MOQ-Secure uses two distinct keying material types:

### Media encryption keys (symmetric)
- For each stream, an **ephemeral symmetric encryption key** is randomly generated.
- Implementations may use **multiple keys per stream** to support key rotation.
- The `keyId` in the frame header selects which symmetric key is used for that frame.

This symmetric key material is delivered to authorized receivers via a **separate key exchange mechanism**, such as:
- a WebSocket / HTTP server, or
- an out-of-band channel (e.g., QR code exchange during onboarding).

### Broadcaster authenticity keys (Ed25519)
- The broadcaster has their own **Ed25519 signing keypair**.
- The corresponding **public Ed25519 key** is publicly associated with that specific broadcaster (for example, through a directory, signed profile, or other public association mechanism).
- Receivers use the broadcaster’s associated public key to verify signed frames.

## Cryptographic primitives

- **Encryption**
  - `ChaCha20-Poly1305` (AEAD)
  - Provides confidentiality + integrity for each frame’s media payload.
  - Benefits:
    - ChaCha20-Poly1305 is efficient on a wide range of platforms, including environments without dedicated AES acceleration.
    - AEAD ensures the payload is both encrypted and integrity-protected in a single step, reducing implementation complexity and protecting against active tampering.

- **Nonce derivation**
  - Nonce is derived from `(keyId, ctr)` to prevent nonce reuse.
  - `ctr` is per-stream: it starts from a random value for each stream and then increments monotonically, enabling safe nonce generation for late join and lossy delivery.

- **Signing**
  - `Ed25519` over a `SHA-256` digest.
  - Benefits:
    - Ed25519 offers fast, compact signatures suitable for per-frame (selective) verification.
    - Ed25519 provides strong authenticity guarantees, enabling receivers to distinguish real broadcaster content from tampered or injected media.

## Signing

Signature verification is optional and selective for performance.

- When signature support is disabled (`nSigned == 0`):
  - `sigFlag` is `0` for all frames
  - No signature trailer is appended
  - Receivers skip signature verification entirely

- When signature support is enabled (`nSigned > 0`):
  - Some frames are sent with `sigFlag = 1` and include the Ed25519 signature trailer
  - The remaining frames are sent unsigned (`sigFlag = 0`) and include no signature bytes
  - Receivers verify signed frames; these serve as periodic refresh points allowing a bounded number of unsigned frames to be accepted

Signature verification excludes the signature bytes themselves from the hashed structure (the signature covers the intended frame content/metadata, with the signature appended as a trailer).

## Wire format details

The complete on-the-wire field layout, byte concatenation rules, nonce/AAD/digest definitions, and receiver processing order live deeper in the repo for those who want the full technical [specification](https://github.com/cathode-ray-tube/moq-secure/blob/main/spec/README.md)

## Usage

This format is intended for implementations that want:
- MOQ’s media-agnostic transport benefits, while
- securing media payload confidentiality and integrity on a per-frame basis,
- optionally adding authenticity for a subset of frames using a broadcaster key.

## Interop

Only encrypts the payload so it should work with any MOQ implementation (e.g., moq-lite, IETF implementations, etc.).

While aimed at MOQ, with some additional wiring it could encrypt any data sent via other transports (such as WebSockets).

## License

This project is dual-licensed: MIT OR Apache-2.0, choose either. See LICENSE-MIT and LICENSE-APACHE-2.0 in the repository root.


