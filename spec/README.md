![moq-secure frame layout](./assets/moq-secure-frame-layout.jpeg)

# MOQ-Secure Encrypted Media Frame Format

### ChaCha20-Poly1305 + Optional Ed25519 Signing

This document defines a **wire format** for encrypted media frames via Media Over QUIC (MOQ).

## Goals

- Encrypt **only the media payload** (transport treats bytes as opaque).
- Use **ChaCha20-Poly1305** for confidentiality + AEAD integrity.
- Optionally provide authenticity via **Ed25519 signatures** on **some frames** using the broadcaster's prior published key.
- Keep a **standard frame layout**:
  - Header includes an `encrypted` flag to support signing-only frames (no AEAD tag).
  - Signatures are appended as an end-of-frame trailer (no fixed `sigSlot` in the header).
- Support lossy delivery + late join:
  - Each frame contains a monotonically incrementing counter (`ctr`) with random starting value.
  - Receiver accepts unsigned frames using a **lease system** based on signed frames.
- Support two encryption modes:
  - **Signing enabled**: `nSigned > 0`
  - **Signing disabled**: `nSigned == 0` (receivers skip signature verification entirely)

> Handshake / out-of-band key exchange is done separately (such as via separate WebSocket/HTTP server or shared in person via QR Code).

---

## 1) Wire Format

Each frame is serialized in this exact order.

### 1.1 Unencrypted Header (parse first)

All integers are **big-endian** unless otherwise stated.

| Field | Size | Description |
|---|---:|---|
| `magic` | 4 bytes | Constant magic value (`MOQS` bytes: `0x4d 0x4f 0x51 0x53`) |
| `version` | 1 byte | Format version (start with `1`) |
| `keyId` | 1 byte | Selects symmetric key (supports key rotation) |
| `ctr` | 8 bytes | Frame counter (`uint64`), random start then incrementing |
| `nSigned` | 1 byte | Lease/signing parameter: `0` disables Ed25519 signing entirely; otherwise lease admission parameter |
| `sigFlag` | 1 byte | `0` = unsigned frame; `1` = signature trailer appended at end |
| `encrypted` | 1 byte | `1` = AEAD encryption used; `0` = signing-only (no AEAD tag; payload is plaintext) |
| `padLen` | 4 bytes | Number of **padding bytes prepended to the plaintext** before encryption (0 allowed). Used to compute usable plaintext bytes. |

**Note:** There is **no fixed signature slot in the header**. If a signature is present, it is appended as a trailer.

### 1.2 Payload area (conditional on `encrypted`)

| Field | Size | Description |
|---|---:|---|
| When `encrypted == 1`: `ciphertext` | N bytes | ChaCha20-Poly1305 ciphertext of padded plaintext |
| When `encrypted == 1`: `aeadTag` | 16 bytes | Poly1305 tag |
| When `encrypted == 0`: `plaintext` | N bytes | Plaintext bytes directly (no AEAD is used) |

For `encrypted == 1`, `ciphertext.length == paddedPlaintext.length`.

Here, `N` includes any padding bytes added by the sender, i.e. padded plaintext length.

### 1.3 Signature trailer (optional, end of frame)

The Ed25519 signature is appended **after** the payload area.

- If `sigFlag == 1`, append a 64-byte `ed25519Signature` trailer.
- If `sigFlag == 0`, append nothing.

Signature bytes are **never included in AEAD AAD**.

---

## 2) Nonce Derivation (12 bytes, derived; not sent)

To avoid nonce reuse, the nonce is derived from (`keyId`, `ctr`).

**Recommended:**
- `nonce12 = SHA256("nonce" || keyId(1) || ctr(8))[0..12)`

Both sender and receiver MUST use the same derivation.

---

## 3) AEAD AAD (Additional Authenticated Data)

When `encrypted == 1`, AAD binds the encrypted payload to the unencrypted header fields.

**AAD definition:**
- `AAD = magic || version || keyId || ctr || nSigned || sigFlag || encrypted || padLen`

All fields are included exactly as they appear in the header, in-order, as raw bytes.

Then, encrypt:
- key selected by `keyId`
- nonce `nonce12`
- AAD as above
- output `ciphertext` + `aeadTag`

For `encrypted == 0`, there is **no AEAD** and therefore no AEAD tag.

---

## 4) Ed25519 Signatures with Lease System

### 4.1 When signing is disabled
If `nSigned == 0`:
- Sender MUST set `sigFlag = 0`
- Sender MUST NOT append any signature trailer
- Receiver MUST NOT verify Ed25519
- Receiver applies **no lease gating** (unsigned frames are accepted)

### 4.2 When signing is enabled
If `nSigned > 0`:
- Some frames will be signed (`sigFlag = 1`) and include an Ed25519 signature trailer.
- Other frames will be unsigned (`sigFlag = 0`) with no signature trailer.

### 4.3 What gets signed
Sign a hash of **everything except** the end-of-frame signature bytes.

Let:
- `headerBytes = magic || version || keyId || ctr || nSigned || sigFlag || encrypted || padLen`
- `payloadBytes =`
  - when `encrypted == 1`: `ciphertext || aeadTag`
  - when `encrypted == 0`: `plaintext` (no `aeadTag`)
- `signedBytes = headerBytes || payloadBytes`
- `digest = SHA256(signedBytes)`

If signing for this frame is enabled (`sigFlag = 1`):
- `signature = Ed25519.Sign(digest, broadcasterPrivateKey)` (64 bytes)
- append `signature` trailer at the end of the frame

If unsigned (`sigFlag = 0`):
- append no signature bytes.

---

## 5) Receiver Verification and Decryption Order

Per frame:

1. Parse header fields:
   `magic, version, keyId, ctr, nSigned, sigFlag, encrypted, padLen`

2. Parse payload area:
   - If `encrypted == 1`: read `ciphertext (N)` and `aeadTag (16)`
   - If `encrypted == 0`: read `plaintext (N)` and note `aeadTag` is absent

3. Signature verification (optional):
   - If `nSigned > 0` and `sigFlag == 1`:
     - recompute `digest = SHA256(headerBytes || payloadBytes)`
     - verify `Ed25519.Verify(digest, signatureTrailer, broadcasterPublicKey)`
     - if verification fails: **drop frame**
   - If `nSigned > 0` and `sigFlag == 0`: no signature verification is performed
   - If `nSigned == 0`: receiver does not verify signatures and must treat `sigFlag` as `0`

4. Lease gating for unsigned frames (only when `nSigned > 0`):
   Receiver maintains `leaseRemaining` (initially 0).
   - If `sigFlag == 1` and verification succeeded:
     - accept frame
     - set `leaseRemaining = nSigned`
   - Else if `sigFlag == 0`:
     - accept only if `leaseRemaining > 0`
     - if accepted: `leaseRemaining -= 1`
     - if `leaseRemaining == 0`: drop frame

5. Decrypt / obtain plaintext:
   - If `encrypted == 1`:
     - derive `nonce12` from (`keyId`, `ctr`)
     - decrypt using ChaCha20-Poly1305 with:
       - key selected by `keyId`
       - nonce `nonce12`
       - AAD = `magic||version||keyId||ctr||nSigned||sigFlag||encrypted||padLen`
     - if AEAD authentication fails: **drop frame**
     - plaintext obtained from decryption (this is the *padded* plaintext)
   - If `encrypted == 0`:
     - plaintext is the payload bytes directly (this is the *padded* plaintext)
     - no AEAD step is performed

6. Compute usable plaintext bytes (padding handling):
   Padding is **prepended** to plaintext prior to encryption.

   Let decrypted/plaintext bytes be `plaintext[0..N-1]`, where `N` includes padding.

   - `usablePlaintext = plaintext[padLen : N]`

7. Deliver `usablePlaintext` to playback/decoder.

---

## 6) Lease System Rationale (lossy UDP + late join)

- UDP may drop frames; receivers may join after stream start.
- Signed frames refresh a receiver’s ability to accept a bounded number of unsigned frames.
- Unsigned frames arriving after the lease expires are discarded until the next valid signed frame arrives.

---

## 7) Size Notes

Let `N` be the **padded plaintext length**, i.e. it already includes any prepended padding bytes.

Fixed header size:
- `magic(4) + version(1) + keyId(1) + ctr(8) + nSigned(1) + sigFlag(1) + encrypted(1) + padLen(4)`
= **21 bytes**

Total frame size:
- `header(20) + payload + optional signature trailer`, where
  - payload is `N + (encrypted==1 ? 16 : 0)` (includes optional padding in N)
  - signature trailer is `(sigFlag==1 ? 64 : 0)`

So:
- **encrypted==1, sigFlag==1:** `21 + (N + 16) + 64 = 101 + N`
- **encrypted==1, sigFlag==0:** `21 + (N + 16) = 37 + N`
- **encrypted==0, sigFlag==1:** `21 + N + 64 = 85 + N`
- **encrypted==0, sigFlag==0:** `21 + N = 21 + N`

---

## Implementation checklist (must match on both sides)

- Same byte serialization order and sizes
- Same nonce derivation from (`keyId`, `ctr`)
- Same AEAD AAD definition: `magic||version||keyId||ctr||nSigned||sigFlag||encrypted||padLen`
- Same signature digest definition:
  - `SHA256(headerBytes || payloadBytes)`
  - where `payloadBytes` is `ciphertext||aeadTag` if encrypted, else `plaintext`
- Same lease logic:
  - `leaseRemaining = nSigned` on each valid signed frame
  - unsigned frames accepted only while `leaseRemaining > 0`
