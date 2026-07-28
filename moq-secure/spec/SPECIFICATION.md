# MOQ-Secure Encrypted Media Frame Format

### ChaCha20-Poly1305 + Optional Ed25519 Leasing

This document defines a **fixed wire format** for encrypted media frames via Media Over QUIC (MOQ).

## Goals

- Encrypt **only the media payload** (transport treats bytes as opaque).
- Use **ChaCha20-Poly1305** for confidentiality + AEAD integrity.
- Optionally provide authenticity via **Ed25519 signatures** on **some frames** using the broadcaster's prior published key.
- Keep a **standard fixed frame layout**:
  - Always include a fixed 64-byte `sigSlot`.
  - When a frame is not signed, `sigSlot` is all zeros.
- Support lossy delivery + late join:
  - Each frame contains a monotonically incrementing counter (`ctr`) with random starting value.
  - Receiver accepts unsigned frames using a **lease system** based on signed frames.
- Support two encryption modes:
  - **Signing enabled**: `nSigned > 0`
  - **Signing disabled** (e.g., password-derived streams): `nSigned == 0`

> Handshake / out-of-band key exchange done separately (such as via separate WebSocket server or shared in person via a QR Code).

---

## 1) Wire Format (fixed)

Each frame is serialized in this exact order.

### 1.1 Unencrypted Header (parse first)

All integers are **big-endian** unless otherwise stated.

| Field | Size | Description |
|---|---:|---|
| `magic` | 4 bytes | Constant magic value (ASCII bytes `MOQS`: 4d 4f 51 53) |
| `version` | 1 byte | Format version (start with `1`) |
| `keyId` | 1 byte | Selects symmetric key (supports key rotation) |
| `ctr` | 8 bytes | Frame counter (`uint64`), random start then incrementing |
| `nSigned` | 1 byte | Lease/signing parameter: `0` disables Ed25519 signing entirely; otherwise lease admission parameter |
| `sigFlag` | 1 byte | `0` = unsigned, `1` = signature present in `sigSlot` |
| `sigSlot` | 64 bytes | Ed25519 signature bytes (64 bytes) if signed, otherwise 64 zero bytes |

**Fixed rule:** `sigSlot` is always present and always exactly 64 bytes.

### 1.2 Encrypted Payload (AEAD)

| Field | Size | Description |
|---|---:|---|
| `ciphertext` | N bytes | ChaCha20-Poly1305 ciphertext of plaintext payload |
| `aeadTag` | 16 bytes | Poly1305 tag (usually appended by implementations) |

**Important:** For ChaCha20-Poly1305, `ciphertext.length == plaintext.length`.

---

## 2) Nonce Derivation (12 bytes, derived; not sent)

To avoid nonce reuse, the nonce is derived from (`keyId`, `ctr`).

Below, "nonce" means ASCII bytes, 6e 6f 6e 63 65.

**Recommended:**
- `nonce12 = SHA256("nonce" || keyId(1) || ctr(8))[0..12)`

Both sender and receiver MUST use the same derivation.

---

## 3) AEAD AAD (Additional Authenticated Data)

AAD binds the encrypted payload to the unencrypted header fields **except** `sigSlot`.

**AAD definition:**
- `AAD = magic || version || keyId || ctr || nSigned || sigFlag`

(`sigSlot` is NOT included in AEAD AAD.)

Then:
- Encrypt plaintext payload using ChaCha20-Poly1305 with:
  - key selected by `keyId`
  - `nonce12`
  - `AAD` as above
  - output `ciphertext` + `aeadTag`

---

## 4) Ed25519 Signatures with Lease System

### 4.1 When signing is disabled
If `nSigned == 0`:
- Sender MUST set:
  - `sigFlag = 0`
  - `sigSlot = 64 zero bytes`
- Receiver MUST NOT verify Ed25519.
- Receiver applies **no lease gating**.

### 4.2 When signing is enabled
If `nSigned > 0`:
- Some frames will be signed (`sigFlag = 1`) and include a valid signature in `sigSlot`.
- Other frames are unsigned (`sigFlag = 0`) and use a zeroed `sigSlot`.

### 4.3 What gets signed
Sign a hash of **everything except the 64-byte `sigSlot`**.

Define:
- `headerWithoutSigSlot = magic || version || keyId || ctr || nSigned || sigFlag`
- `signedBytes = headerWithoutSigSlot || ciphertext || aeadTag`
- `digest = SHA256(signedBytes)`

If signing for this frame is enabled:
- `signature = Ed25519.Sign(digest, broadcasterPrivateKey)` (64 bytes)
- `sigFlag = 1`
- `sigSlot = signature`

For unsigned frames:
- `sigFlag = 0`
- `sigSlot = 64 zero bytes`

---

## 5) Receiver Verification Order

Receiver behavior per frame:

1. Parse header:
   - read `magic`, `version`, `keyId`, `ctr`, `nSigned`, `sigFlag`, `sigSlot`
2. Read `ciphertext` and `aeadTag`
3. **If `nSigned > 0` and `sigFlag == 1`:**
   - recompute `digest = SHA256(headerWithoutSigSlot || ciphertext || aeadTag)`
   - verify `Ed25519.Verify(digest, sigSlot, broadcasterPublicKey)`
   - if verification fails: **drop frame**
4. **Lease gating for unsigned frames** (only when `nSigned > 0`):
   - Receiver maintains `leaseRemaining` (initially 0)
   - If the frame is signed and verification succeeded:
     - accept frame
     - set `leaseRemaining = nSigned`
   - If the frame is unsigned (`sigFlag == 0`):
     - accept only if `leaseRemaining > 0`
     - if accepted: `leaseRemaining -= 1`
     - if `leaseRemaining == 0`: drop frame
5. Decrypt:
   - derive `nonce12` from (`keyId`, `ctr`)
   - decrypt using ChaCha20-Poly1305 with:
     - key selected by `keyId`
     - nonce `nonce12`
     - AAD = `magic || version || keyId || ctr || nSigned || sigFlag`
   - if AEAD authentication fails: **drop frame**
6. Deliver plaintext to playback/decoder.

---

## 6) Lease System Rationale (lossy UDP + late join)

- UDP may drop frames; receivers may join after stream start.
- Signed frames refresh a receiver’s ability to accept a bounded number of unsigned frames.
- If unsigned frames arrive after the lease expires, they are discarded until the next valid signed frame arrives.

---

## 7) Size Notes

Fixed overhead excluding ciphertext:
- `magic(4) + version(1) + keyId(1) + ctr(8) + nSigned(1) + sigFlag(1) + sigSlot(64) + aeadTag(16)`
= **96 bytes + ciphertext length (N)**

---

## Implementation checklist (must match on both sides)

- Same byte serialization order and sizes
- Same nonce derivation from (`keyId`, `ctr`)
- Same AEAD AAD definition: `magic||version||keyId||ctr||nSigned||sigFlag`
- Signature digest definition:
  - SHA256(headerWithoutSigSlot || ciphertext || aeadTag)
  - where `headerWithoutSigSlot` excludes the `sigSlot` bytes
- Same lease logic:
  - `leaseRemaining = nSigned` on each valid signed frame
  - unsigned frames accepted only while `leaseRemaining > 0`
