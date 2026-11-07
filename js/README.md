# @cathode-ray-tube/moq-secure

Encryption and signing utilities for complete application payloads.

This package is maintained separately from the upstream MoQ JavaScript
packages. The corresponding secure publish and watch packages are:

- [`@cathode-ray-tube/moq-secure-publish`](https://www.npmjs.com/package/@cathode-ray-tube/moq-secure-publish)
- [`@cathode-ray-tube/moq-secure-watch`](https://www.npmjs.com/package/@cathode-ray-tube/moq-secure-watch)

## Install

```bash
npm install @cathode-ray-tube/moq-secure
```

## Encrypt and decrypt a payload

The example below shows the API flow. Your application must provision the
AEAD key and signing key securely. The receiver needs the corresponding
broadcaster public key and the same AEAD key for the selected `keyId`.

```js
import {
  EncryptionType,
  InMemoryKeyStore,
  MoqSecureDecrypter,
  MoqSecureEncrypter,
} from "@cathode-ray-tube/moq-secure";

// Obtain these from your application's secure key provisioning mechanism.
// Do not hard-code production keys or commit them to source control.
const aeadKey = getAeadKey(); // 32-byte Uint8Array
const signingPrivateKey = getSigningPrivateKey();
const broadcasterPublicKey = getBroadcasterPublicKey();

const keyId = 1;
const keyStore = new InMemoryKeyStore();
keyStore.setKey(keyId, aeadKey);

const encrypter = new MoqSecureEncrypter({
  keyStore,
  signingPrivateKey,
  keyId,
  encryptionType: EncryptionType.AES_256_GCM,
  nSigned: 1, // Sign every frame; use 0 to disable signing.
  padLen: 0,
});

// Supply the application payload in the format your integration expects.
const payload = getPayload();

const encryptedFrame = await encrypter.encrypt(payload);

const decrypter = new MoqSecureDecrypter({
  keyStore,
  broadcasterPublicKey,
});

const decrypted = await decrypter.decrypt(encryptedFrame);
// `decrypted` contains the original payload.
```

`getAeadKey()`, `getSigningPrivateKey()`, `getBroadcasterPublicKey()`, and
`getPayload()` are placeholders for functions supplied by your application;
they are not exports from this package.

`InMemoryKeyStore.setKey(keyId, key)` accepts a key ID from `0` to `255` and
a 32-byte AEAD key. Alternatively, `setKeyEncoded(keyId, encoded)` accepts a
32-byte key encoded as hexadecimal or base64.

## Signing cadence

`nSigned` controls which frames are signed:

- `0`: signing disabled
- `1`: every frame signed
- `2`: every second frame
- `3`: every third frame

The schedule starts with the first frame. For example, `nSigned: 2` signs
frames 1, 3, 5, and so on.

## Playback and replay handling

The decrypter tracks counters separately for each key ID and rejects
replayed or out-of-order counters within the current playback mode.

For live playback, call:

```js
decrypter.beginLive();
```

At the start of a rewind invocation, call:

```js
decrypter.beginRewind();
```

`beginRewind()` resets the rewind playback cursor while retaining the live
counter high-water marks. Call `beginLive()` when returning to live playback.

## Encryption types

`EncryptionType` provides these wire-header identifiers:

- `EncryptionType.NONE`
- `EncryptionType.CHACHA20_POLY1305`
- `EncryptionType.AES_256_GCM` (the default)

These numeric identifiers are protocol values and must remain stable.

## Development

From the JavaScript package directory:

```bash
npm install
npm run build
npm run typecheck
npm test
```

## License

Dual-licensed under either of the following, at your option:

- MIT
- Apache License 2.0

See [`LICENSE-MIT`](LICENSE-MIT) and
[`LICENSE-APACHE-2.0`](LICENSE-APACHE-2.0) for the license texts.

