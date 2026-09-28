# moq-secure-chat

A browser-based AES-256-GCM encrypted chat example using MoQ, `moq-secure`,
and a custom `<moq-secure-chat>` web component.

The example demonstrates:

- Generating a 32-byte AES-256 encryption key
- Generating an Ed25519 signing key pair
- Publishing AES-256-GCM encrypted and signed chat messages over MoQ
- Subscribing to another broadcaster
- Verifying signatures and decrypting messages in the browser
- Encoding keys as Base64 for sharing between participants

## Cryptography

Chat frames use:

- **AES-256-GCM** for authenticated encryption
- **Ed25519** for message signatures
- **128-bit authentication tags**
- **Zero padding** configured by the example's secure encrypter

The encryption key is 32 bytes long and is shared with subscribers so they can
decrypt the broadcaster's messages. The signing private key remains with the
publisher. Subscribers receive only the corresponding signing public key.

## Run

From the repository root:

```bash
npm install
npm run dev
```

Open the URL printed by Vite, usually:

```text
http://localhost:5173/
```

The example is located in:

```text
examples/moq-secure-chat/
```

## Use

### 1. Generate an identity

Click **Generate keys**.

This creates:

- An AES-256 encryption key used to encrypt chat frames
- A signing private key used by the publisher
- A signing public key used by subscribers to verify messages

The generated keys are displayed as Base64 values.

### 2. Publish messages

Enter:

- **Relay URL** — the MoQ relay endpoint
- **Broadcast name** — the broadcast or room name

Click **Start publishing**, then send messages using the composer.

Messages are encrypted with AES-256-GCM before being published. Frames are also
signed using Ed25519 according to the configured signing schedule. Subscribers
use the broadcaster's public key to verify the signatures.

### 3. Subscribe to a broadcast

Enter:

- The relay URL
- The broadcast name
- The broadcaster's encryption key
- The broadcaster's signing public key

Click **Add subscription**.

Incoming frames are:

1. Parsed from the wire format
2. Authenticated and decrypted with AES-256-GCM
3. Signature-verified with Ed25519
4. Decoded as chat messages
5. Displayed in the chat window

## Key sharing

To allow another browser to subscribe, share these values from the publisher:

- **Encryption key**
- **Signing public key**
- Relay URL
- Broadcast name

Never share the signing private key. It is used only by the publisher to sign
frames.

The encryption key must be shared with subscribers because AES-GCM encryption
is symmetric: the same key is required for decryption.

## Custom element

The application is registered as:

```html
<moq-secure-chat></moq-secure-chat>
```

The component manages identity generation, publishing, subscriptions, message
encoding, and rendering.

## Main components

- `secure-factory.ts` — creates identities and configures AES-256-GCM secure codecs
- `moq-chat.ts` — connects publishers and subscribers to MoQ
- `secure-chat.ts` — encrypts and decrypts chat messages
- `encrypter.ts` — creates AES-256-GCM encrypted `moq-secure` frames
- `decrypter.ts` — verifies and decrypts frames
- `wire.ts` — serializes, signs, parses, and authenticates frames
- `crypto.ts` — provides AES-256-GCM and ChaCha20-Poly1305 operations
- `constants.ts` — defines wire-format constants and encryption type identifiers
- `types.ts` — defines shared message, identity, and subscription types

## Production considerations

This is an example application. It does not persist identities or keys, and
refreshing the page may require generating or entering the keys again.

Production use would require additional hardening, including secure key
storage, authenticated key distribution, access control, key rotation,
replay protection, and appropriate handling of relay and broadcast metadata.
