![moq-secure frame layout](https://github.com/cathode-ray-tube/moq-secure/blob/main/assets/moq-secure-frame-layout.jpeg)

### ChaCha20-Poly1305 + Optional Ed25519 Signing

This document defines a **wire format** for encrypted media payloads via Media Over QUIC (MOQ).

## Goals

- Encrypt **only the media payload** (transport treats bytes as opaque).
- Entire MOQ-Secure frame is nested within the MOQ frame payload.
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
- Flexibility around user requirements, balancing security and performance.

> Handshake / out-of-band key exchange is done separately (such as via separate WebSocket/HTTP server or shared in person via QR Code).

---

## 1) Wire Format

Each frame is serialized in this exact order.

### 1.1 Unencrypted Header (parse first)

All integers are big-endian unless otherwise stated.

| Field | Type | Size | Description |
|---|---|---:|---|
| `magic` | `bytes[4]` | 4 bytes | Constant ASCII magic value (`MOQS` bytes: `0x4d 0x4f 0x51 0x53`) |
| `version` | `uint8` | 1 byte | Format version (start with `1`) |
| `keyId` | `uint8` | 1 byte | Selects symmetric key (supports key rotation) |
| `ctr` | `uint64` | 8 bytes | Frame counter (`uint64`), random start then incrementing |
| `nSigned` | `uint8` | 1 byte | Lease/signing parameter: `0` disables Ed25519 signing entirely; otherwise lease admission parameter |
| `sigFlag` | `uint8` | 1 byte | `0` = unsigned frame; `1` = signature trailer appended at end |
| `encrypted` | `uint8` | 1 byte | `1` = AEAD encryption used; `0` = signing-only (no AEAD tag; payload is plaintext) |
| `padLen` | `uint32` | 4 bytes | Number of padding bytes prepended to the plaintext before encryption (0 allowed). Used to compute usable plaintext bytes. |

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
### 2) Nonce Derivation (12 bytes, derived; not sent)

To avoid nonce reuse, the nonce is derived from (`keyId`, `ctr`).

**Recommended:**
- Let `nonceTag =` the ASCII bytes of the string `"nonce"`:
  - `nonceTag = 0x6e 0x6f 0x6e 0x63 0x65`
- `nonce12 = SHA256(nonceTag || keyId(1) || ctr(8))[0..12)`

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
- `nSigned` value indicates signing frequency:
  - `nSigned = 1` would mean **every** frame is signed.
  - `nSigned = 30` would mean **only every 30th** frame is signed.
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


--------------------

User prompt 2 of 2 - 8/28/2026, 12:28:13 AM:
here is the frame format:  magic(4)
version(1)
key_id(1)
ctr(8)
n_signed(1)
sig_flag(1)
encrypted(1)
payload
tag(16, encrypted frames only)
signature(64, signed frames only)

payload:

pad_len(4-byte big-endian)
zero padding(pad_len)
plaintext

GPT-5.6 Luna:
<!-- **Updating README**

I'm focusing on producing an updated README. I need to clear up some ambiguities regarding the payload: it should include the padlen, padding, and plaintext, with the tag coming after the ciphertext. If it's unencrypted, the plaintext structure needs to come after the signature. The header is 17 bytes in total. The AAD will exclude the padlen since it's encrypted. I need to confirm if the signature covers the entire header and payload area, including the tag. Alright, time to write a clear and concise README! -->
# ChaCha20-Poly1305 + Optional Ed25519 Signing

This document defines a wire format for encrypted media payloads transmitted via Media Over QUIC (MOQ).

## Goals

- Encrypt only the media payload; the transport treats the bytes as opaque.
- Nest the entire MOQ-Secure frame inside the MOQ frame payload.
- Use ChaCha20-Poly1305 for confidentiality and AEAD integrity.
- Optionally provide authenticity through Ed25519 signatures on selected frames.
- Support signing-only frames without an AEAD tag.
- Support lossy delivery and late joining through a frame counter and signed-frame lease system.
- Allow signing to be disabled entirely with `n_signed == 0`.
- Keep all variable-length payload data inside the encrypted portion of encrypted frames.

Handshake and out-of-band key exchange are performed separately, such as through a WebSocket/HTTP server or by sharing keys in person using a QR code.

---

## 1. Wire Format

All integer fields are big-endian unless otherwise stated.

Each frame is serialized in this order:

```text
header
payload
optional AEAD tag
optional Ed25519 signature
```

### 1.1 Unencrypted header

The header is always 17 bytes and is parsed before processing the payload.

| Field | Type | Size | Description |
|---|---|---:|---|
| `magic` | `bytes[4]` | 4 bytes | Constant ASCII magic value `MOQS`: `0x4d 0x4f 0x51 0x53` |
| `version` | `uint8` | 1 byte | Wire-format version; initially `1` |
| `key_id` | `uint8` | 1 byte | Selects the symmetric key and supports key rotation |
| `ctr` | `uint64` | 8 bytes | Frame counter; randomly initialized, then incremented |
| `n_signed` | `uint8` | 1 byte | Signing and lease parameter |
| `sig_flag` | `uint8` | 1 byte | `0` = no signature; `1` = 64-byte signature follows the payload |
| `encrypted` | `uint8` | 1 byte | `1` = ChaCha20-Poly1305; `0` = signing-only/plaintext |

The header does **not** contain `pad_len`. The `pad_len` field is the first four bytes of the payload and is encrypted when `encrypted == 1`.

### 1.2 Payload format

The logical payload is always:

```text
pad_len       (4 bytes, big-endian)
zero_padding  (pad_len bytes)
plaintext     (remaining bytes)
```

`pad_len` specifies the number of zero-padding bytes prepended to the plaintext.

#### Encrypted frames

For encrypted frames, the complete logical payload is encrypted:

```text
ciphertext = ChaCha20-Poly1305(
    key,
    nonce,
    payload,
    AAD
)
```

The frame contains:

```text
header
ciphertext
aead_tag
optional signature
```

The ciphertext includes the encrypted `pad_len` field, zero padding, and plaintext. Its length is equal to the unencrypted logical payload length.

The AEAD tag is 16 bytes.

#### Signing-only frames

For frames where `encrypted == 0`, no AEAD operation is performed. The frame contains:

```text
header
pad_len
zero_padding
plaintext
optional signature
```

No AEAD tag is present.

---

## 2. Nonce Derivation

The 12-byte ChaCha20-Poly1305 nonce is derived from `key_id` and `ctr`; it is not transmitted in the frame.

Use the following derivation:

```text
nonce_tag = ASCII("nonce")
nonce_input = nonce_tag || key_id(1 byte) || ctr(8 bytes)
nonce12 = SHA256(nonce_input)[0..12)
```

In hexadecimal, `nonce_tag` is:

```text
6e 6f 6e 63 65
```

Both the sender and receiver MUST use the same nonce derivation.

A given `(key_id, ctr)` pair MUST NOT be reused with the same symmetric key.

---

## 3. AEAD Additional Authenticated Data

When `encrypted == 1`, the AEAD additional authenticated data is the complete unencrypted header:

```text
AAD =
    magic ||
    version ||
    key_id ||
    ctr ||
    n_signed ||
    sig_flag ||
    encrypted
```

`pad_len` is **not** included in the AAD because it is part of the encrypted payload.

The sender encrypts using:

- the symmetric key selected by `key_id`;
- the derived `nonce12`;
- the AAD defined above;
- the logical payload:

```text
pad_len || zero_padding || plaintext
```

The result is:

```text
ciphertext || aead_tag
```

For `encrypted == 0`, no AEAD operation or AEAD tag is used.

---

## 4. Ed25519 Signatures

A signature, when present, is appended after the payload and after the AEAD tag for encrypted frames.

### 4.1 Signature trailer

If `sig_flag == 1`:

```text
ed25519_signature
```

A signature is exactly 64 bytes.

If `sig_flag == 0`, no signature bytes are appended.

There is no fixed signature slot in the header.

### 4.2 Data covered by the signature

The signature covers everything in the frame except the signature itself.

For an encrypted frame:

```text
signed_bytes = header || ciphertext || aead_tag
```

For an unencrypted frame:

```text
signed_bytes = header || pad_len || zero_padding || plaintext
```

The sender computes:

```text
digest = SHA256(signed_bytes)
signature = Ed25519.Sign(digest, broadcaster_private_key)
```

The resulting 64-byte signature is appended to the frame.

Signature bytes are not included in AEAD AAD.

---

## 5. Signing Modes and Lease System

### 5.1 Signing disabled

When `n_signed == 0`:

- The sender MUST set `sig_flag == 0`.
- The sender MUST NOT append a signature.
- The receiver MUST NOT perform Ed25519 verification.
- The receiver MUST NOT apply lease gating.
- Frames are accepted based on the encryption and AEAD rules only.

A frame with `n_signed == 0` and `sig_flag == 1` is invalid and SHOULD be dropped.

### 5.2 Signing enabled

When `n_signed > 0`:

- Signed frames have `sig_flag == 1`.
- Unsigned frames have `sig_flag == 0`.
- A signed frame refreshes the receiver's lease.
- An unsigned frame is accepted only while the receiver has remaining lease credits.

`n_signed` determines the signing interval:

- `n_signed == 1`: every frame is signed.
- `n_signed == 30`: one frame is signed for every 30-frame interval, depending on the sender's scheduling policy.

The sender SHOULD use a consistent signing schedule for a stream.

### 5.3 Lease behavior

The receiver maintains:

```text
lease_remaining
```

The initial value is zero.

For a valid signed frame:

```text
lease_remaining = n_signed
```

For an unsigned frame:

- accept the frame only if `lease_remaining > 0`;
- after accepting it, decrement `lease_remaining` by one;
- drop it if `lease_remaining == 0`.

This allows signed frames to authorize a bounded number of subsequent unsigned frames while limiting the effect of unauthenticated data.

---

## 6. Receiver Processing

The receiver processes each frame as follows.

### Step 1: Parse the header

Read:

```text
magic
version
key_id
ctr
n_signed
sig_flag
encrypted
```

Validate the magic, supported version, and flag values.

The header length is 17 bytes.

### Step 2: Determine the frame sections

The frame layout is:

```text
17-byte header
payload
optional 16-byte AEAD tag
optional 64-byte signature
```

The receiver must know the enclosing MOQ payload boundary so it can determine the payload length.

If `sig_flag == 1`, reserve the final 64 bytes for the signature trailer.

If `encrypted == 1`, reserve the 16 bytes immediately before the optional signature for the AEAD tag.

The remaining bytes are the payload or ciphertext.

### Step 3: Verify the signature, when enabled

If `n_signed > 0` and `sig_flag == 1`:

1. Compute `signed_bytes` as the header plus the payload area and, for encrypted frames, the AEAD tag.
2. Compute:

   ```text
   digest = SHA256(signed_bytes)
   ```

3. Verify the signature using the broadcaster's previously published Ed25519 public key.
4. Drop the frame if verification fails.

If `n_signed > 0` and `sig_flag == 0`, do not perform signature verification.

If `n_signed == 0`, `sig_flag` must be zero and no signature is processed.

### Step 4: Apply lease gating

When `n_signed > 0`:

- For a successfully verified signed frame:
  - accept the frame;
  - set `lease_remaining = n_signed`.
- For an unsigned frame:
  - accept only if `lease_remaining > 0`;
  - decrement `lease_remaining` after accepting it;
  - otherwise drop the frame.

When `n_signed == 0`, skip lease gating.

### Step 5: Decrypt or obtain the logical payload

For an encrypted frame:

1. Derive `nonce12` from `key_id` and `ctr`.
2. Use the symmetric key selected by `key_id`.
3. Use the header as AAD:

   ```text
   magic || version || key_id || ctr || n_signed || sig_flag || encrypted
   ```

4. Decrypt and authenticate the ciphertext using the AEAD tag.
5. Drop the frame if authentication fails.

The decrypted result is:

```text
pad_len || zero_padding || plaintext
```

For an unencrypted frame, the payload is already:

```text
pad_len || zero_padding || plaintext
```

### Step 6: Parse padding and deliver plaintext

Read the first four bytes of the logical payload as a big-endian `uint32`:

```text
pad_len
```

Let the remaining logical payload length be `N`.

The receiver MUST verify:

```text
pad_len <= N - 4
```

The zero-padding region is:

```text
logical_payload[4 .. 4 + pad_len)
```

The usable plaintext begins after the four-byte `pad_len` field and the padding:

```text
usable_plaintext =
    logical_payload[4 + pad_len .. N)
```

The receiver SHOULD verify that every byte in the padding region is zero. If the padding is nonzero, the frame SHOULD be dropped.

Deliver `usable_plaintext` to the playback or decoding pipeline.

---

## 7. Frame Size

The fixed header is:

```text
magic       4 bytes
version     1 byte
key_id      1 byte
ctr         8 bytes
n_signed    1 byte
sig_flag    1 byte
encrypted   1 byte
                         = 17 bytes
```

Let:

- `P` = plaintext length;
- `pad_len` = number of zero-padding bytes;
- `N = 4 + pad_len + P` = logical payload length, including the four-byte `pad_len` field;
- AEAD tag size = 16 bytes;
- Ed25519 signature size = 64 bytes.

Total frame sizes are:

| `encrypted` | `sig_flag` | Total size |
|---:|---:|---:|
| `1` | `1` | `17 + N + 16 + 64 = 97 + N` |
| `1` | `0` | `17 + N + 16 = 33 + N` |
| `0` | `1` | `17 + N + 64 = 81 + N` |
| `0` | `0` | `17 + N` |

Substituting `N = 4 + pad_len + P`:

| `encrypted` | `sig_flag` | Total size |
|---:|---:|---:|
| `1` | `1` | `101 + pad_len + P` |
| `1` | `0` | `37 + pad_len + P` |
| `0` | `1` | `85 + pad_len + P` |
| `0` | `0` | `21 + pad_len + P` |

---

## 8. Invalid Frames

A receiver SHOULD drop a frame if any of the following conditions apply:

- `magic` is incorrect.
- `version` is unsupported.
- `encrypted` is not `0` or `1`.
- `sig_flag` is not `0` or `1`.
- `n_signed == 0` but `sig_flag == 1`.
- The frame is too short for its declared tag or signature.
- A required signature fails Ed25519 verification.
- An encrypted frame fails AEAD authentication.
- The logical payload is shorter than four bytes.
- `pad_len` exceeds the available payload after the four-byte length field.
- The padding bytes are not all zero.
- The frame violates the applicable lease rules.

---

## Implementation Checklist

Both sender and receiver MUST agree on:

- Header field order and sizes:

  ```text
  magic(4) ||
  version(1) ||
  key_id(1) ||
  ctr(8) ||
  n_signed(1) ||
  sig_flag(1) ||
  encrypted(1)
  ```

- Big-endian encoding for `ctr` and `pad_len`.
- The logical payload format:

  ```text
  pad_len(4) || zero_padding(pad_len) || plaintext
  ```

- Nonce derivation:

  ```text
  SHA256("nonce" || key_id || ctr)[0..12)
  ```

- AEAD AAD:

  ```text
  header
  ```

- Encrypted-frame layout:

  ```text
  header || ciphertext || aead_tag || optional_signature
  ```

- Unencrypted-frame layout:

  ```text
  header || pad_len || zero_padding || plaintext || optional_signature
  ```

- Signature input:
  - encrypted: `header || ciphertext || aead_tag`;
  - unencrypted: `header || payload`.

- Ed25519 signing of `SHA256(signed_bytes)`.
- Signature and AEAD tag sizes:
  - Ed25519 signature: 64 bytes;
  - Poly1305 tag: 16 bytes.
- Lease handling:
  - valid signed frame sets `lease_remaining = n_signed`;
  - unsigned frames consume one lease credit;
  - unsigned frames are rejected when no credits remain.
