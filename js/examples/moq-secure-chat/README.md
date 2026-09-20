# moq-secure-chat

A browser-based encrypted chat example using MoQ, `moq-secure`, and a custom
`<moq-secure-chat>` web component.

The example demonstrates:

- Generating an encryption key and Ed25519 signing key pair
- Publishing encrypted and signed chat messages over MoQ
- Subscribing to another broadcaster
- Verifying signatures and decrypting messages in the browser
- Encoding keys as Base64 for sharing between participants

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

- An encryption key used to encrypt chat frames
- A signing private key used by the publisher
- A signing public key used by subscribers to verify messages

The generated keys are displayed as Base64 values.

### 2. Publish messages

Enter:

- **Relay URL** — the MoQ relay endpoint
- **Broadcast name** — the broadcast or room name

Click **Start publishing**, then send messages using the composer.

Messages are encrypted before publishing. With signing enabled, subscribers can
also verify that frames came from the broadcaster.

### 3. Subscribe to a broadcast

Enter:

- The relay URL
- The broadcast name
- The broadcaster's encryption key
- The broadcaster's signing public key

Click **Add subscription**.

Incoming messages are decrypted, authenticated, decoded, and displayed in the
chat window.

## Key sharing

To allow another browser to subscribe, share these values from the publisher:

- **Encryption key**
- **Signing public key**
- Relay URL
- Broadcast name

Never share the signing private key. It is used only by the publisher to sign
frames.

## Custom element

The application is registered as:

```html
<moq-secure-chat></moq-secure-chat>
```

The component manages identity generation, publishing, subscriptions, message
encoding, and rendering.

## Main components

- `secure-factory.ts` — creates identities and secure codecs
- `moq-chat.ts` — connects publishers and subscribers to MoQ
- `secure-chat.ts` — encrypts and decrypts chat messages
- `encrypter.ts` — creates encrypted moq-secure frames
- `decrypter.ts` — verifies and decrypts frames
- `wire.ts` — serializes, signs, parses, and authenticates frames
- `types.ts` — shared message, identity, and subscription types

## Production

This is an example application. It does not persist identities or keys, and
refreshing the page may require generating or entering the keys again. **Would require hardening for production usage.**
