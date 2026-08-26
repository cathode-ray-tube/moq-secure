# moq-secure tests

This directory contains the Vitest test suite for `moq-secure`.

The tests cover:

- AEAD encryption and decryption
- SHA-256 hashing
- Deterministic nonce derivation
- In-memory key storage
- Hexadecimal and Base64 key decoding
- Defensive copying of key material
- Zero-padding helpers
- Async iterable and `ReadableStream` conversions
- Wire-header encoding and validation
- Frame parsing and serialization
- Encrypted and cleartext frames
- Ed25519 frame signatures
- Signature lease handling
- Authentication, truncation, and key errors

## Test files

| File | Coverage |
| --- | --- |
| `crypto.test.ts` | SHA-256, ChaCha20-Poly1305, authentication failures, nonce usage |
| `keys-padding.test.ts` | Key storage, key decoding, validation, and padding |
| `streams.test.ts` | Async iterables, readable streams, transforms, collection, and errors |
| `wire.test.ts` | Headers, frames, encryption, signatures, leases, and parsing errors |

## Running the tests

Run the complete test suite from the package root `/js`:

```bash
npm test
```
