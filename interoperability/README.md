## Interoperability

The Rust and TypeScript/JavaScript versions are meant to be fully compatible, as are the browser and Node.js builds.

The following points highlight subtle differences uncovered during development. They serve as a quick reference for anyone modifying the core library or porting it to another language, helping avoid common pitfalls.

### 1. Async cryptographic APIs  

**Rust (synchronous)**  

```rust
signing_key.sign(&digest)
verifying_key.verify(&digest, &signature)
```  

**TypeScript (asynchronous)**  

```ts
const frame = await encryptFrame(...);
const plaintext = await decryptFrame(...);
```  

*The API difference does **not** affect the wire format.*  

---  

### 2. `u64` versus JavaScript numbers  

**Rust max value**  

```
18,446,744,073,709,551,615
```  

**TypeScript** – use `bigint` for counters:  

```ts
ctr: bigint;
```  

Calling code:  

```ts
const ctr = 123n;
```  

*Converting through `Number` can corrupt nonces and break decryption.*  

---  

### 3. Byte‑array types  

**Rust**  

```rust
&[u8]          // slice
[u8; 32]       // fixed array
[u8; 64]       // fixed array
```  

**TypeScript** – use `Uint8Array` (mutable, so validate lengths at every public boundary):  

| Field | Length (bytes) |
|-------|----------------|
| AEAD key | 32 |
| Nonce | 12 |
| AEAD tag | 16 |
| Ed25519 public key | 32 |
| Ed25519 private key (seed) | 32 |
| Signature | 64 |

*Copy inputs when storing them to prevent accidental mutation.*  

---  

### 4. Buffer vs. Uint8Array  

- Node code often uses `Buffer`; browser code should not depend on it.  
- Core implementation should **only** use:  

```ts
Uint8Array
DataView
ArrayBuffer
```  

Node‑specific conversions belong in optional adapters.  

---  

### 5. Integer encoding  

**Rust (big‑endian)**  

```rust
ctr.to_be_bytes()
pad_len.to_be_bytes()
```  

**TypeScript** – explicit big‑endian writes:  

```ts
view.setBigUint64(offset, value, false);   // 64‑bit
view.setUint32(offset, value, false);      // 32‑bit
```  

*The final `false` forces big‑endian; using little‑endian breaks the wire format.*  

---  

### 6. Unsigned integer validation  

| Field | Valid range |
|-------|-------------|
| `keyId` | 0 .. 255 |
| `nSigned` | 0 .. 255 |
| `sigFlag` | 0 or 1 |
| `encrypted` | 0 or 1 |
| `padLen` | 0 .. 4,294,967,295 (`u32`) |
| `ctr` | 0 .. 2⁶⁴‑1 (`bigint`) |

*JavaScript can safely represent the full `u32` range; use `bigint` for `ctr`.*  

---  

### 7. Array slicing semantics  

- `array.slice()` → **copy**  
- `array.subarray()` → **view**  

*For parsed frames and stored keys, `slice()` is generally safer to avoid later mutation.*  

---  

### 8. AEAD ciphertext & tag representation  

**Rust** returns `ciphertext || tag` and splits into:  

```rust
ciphertext: Vec<u8>
tag: [u8; 16]
```  

**TypeScript** must keep the same distinction and **must not**:  

- Include the tag in the payload  
- Omit the tag during serialization  
- Authenticate the tag separately  
- Swap order (`tag || ciphertext`)  

**Wire format**:  

```
header || ciphertext || tag || optional_signature
```  

---  

### 9. AEAD AAD (Additional Authenticated Data)  

Exactly the encoded 28‑byte header:  

```
magic
version
key_id
ctr
n_signed
sig_flag
encrypted
pad_len
```  

*The signature trailer is **never** part of the AAD.*  

---  

### 10. Nonce derivation (byte‑for‑byte identical)  

```
nonce = SHA‑256("nonce" || key_id || ctr.to_be_bytes())[0..12]
```  

- The string is the ASCII bytes `6e 6f 6e 63 65`.  
- Counter must be eight big‑endian bytes.  

*Common pitfalls:* UTF‑16 encoding of “nonce”, little‑endian counters, hashing a decimal string, using the full SHA‑256 output, treating `keyId` as multi‑byte.  

---  

### 11. Padding behavior  

```ts
padLen === 0   // plaintext is used directly
```  

If `padLen > 0`:  

```
zero bytes × padLen || plaintext
```  

- **Encrypted mode:** padded value is encrypted.  
- **Unencrypted mode:** padded value is stored directly.  

Padding is removed only after decryption or direct payload retrieval.  

---  

### 12. Error handling  

Provide a custom error class with stable codes:  

```ts
try {
  await decryptFrame(...);
} catch (error) {
  if (
    error instanceof MoqSecureError &&
    error.code === "AeadAuthFailed"
  ) {
    // handle authentication failure
  }
}
```  

*Don’t rely on matching error‑message strings.*  

---  

### 13. Signature verification input  

- **Encrypted frames:**  

```
SHA‑256( encoded_header || ciphertext || tag )
```  

- **Unencrypted frames:**  

```
SHA‑256( encoded_header || padded_plaintext )
```  

*The 64‑byte signature trailer itself is excluded from the digest.*  

---  

### 14. Ed25519 key formats  

```ts
privateKey: Uint8Array // 32‑byte seed
publicKey: Uint8Array  // 32‑byte public key
signature: Uint8Array  // 64‑byte signature
```  

*Do not silently accept PKCS#8, PEM, hex, or Base64 unless you add explicit converters.*  

---  

### 15. Lease mutation & failed frames  

```ts
const lease = { remaining: 0 };
```  

- Signed frame **resets** lease.  
- Unsigned frame **decrements** lease.  

*Verify signature before mutating the lease; an AEAD failure should not consume a lease unit unless the protocol dictates.*  

Concurrent consumers must protect the lease object themselves.  

---  

### 16. Stream semantics  

Portable abstraction:  

```ts
AsyncIterable<Uint8Array>
```  

Adapters can support:  

- Web `ReadableStream`  
- Node.js readable streams  
- Rust/WASM async streams  
- JS async generators  

**Important stream properties:** backpressure, cancellation, cleanup in `finally`, error propagation, avoiding full‑buffering, preserving chunk boundaries only when required.  

---  

### 17. WASM interop  

When Rust compiles to WebAssembly, bindings may expose:  

- `Uint8Array` buffers  
- Copied byte arrays  
- Pointers & lengths  
- Promises / callbacks  
- Rust stream wrappers  
- Web Streams  

*Copy data unless ownership/lifetime is explicit; a Rust‑owned buffer may be invalid after the call.*  

---  

### 18. Browser & Node packaging  

Core code **must not** import Node‑only modules:

```ts
import { Buffer } from "node:buffer";
import { Readable } from "node:stream";
```  

Use conditional exports, e.g.:

```
moq-secure
moq-secure/node
moq-secure/browser
```  

Target `tsconfig`:

```json
{
  "lib": ["ES2022", "DOM", "DOM.Iterable"]
}
```  

---  

### 19. Randomness  

Nonce is deterministic (`keyId` + `ctr`).  Counter uniqueness is **critical** – never reuse the same `(keyId, ctr)` pair with ChaCha20‑Poly1305.

Care should be taken during any key-generation additions to the code, ensuring randomness.

---  

### 20. Tests

**Test vectors** can be generated, from repo root:

```bash
**Test vectors** can be generated, from repo root:

```bash
npm install
npm run vectors:generate
```

This will populate the `frames.json` file in the `test-vectors` directory.

Run **rust** tests, from repo root:

```bash
cargo test
```

Run **javascript** tests, from repo root:

```bash
npm test
```

*Wire format and cryptographic bytes are the compatibility boundaries; internal class names, module layout, or async vs sync APIs may differ.* 
