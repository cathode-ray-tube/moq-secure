# Basic JavaScript Example

This example demonstrates a basic AES-256-GCM encrypted `moq-secure` frame round trip:

1. Generate a 32-byte AES-256 encryption key.
2. Store the key in an in-memory key store.
3. Encrypt plaintext into a `Frame` using AES-256-GCM.
4. Serialize the frame to bytes.
5. Decrypt the serialized frame.
6. Print the recovered plaintext.

The example uses encryption without signatures. The `nSigned` value is `0`, so the placeholder signing keys are not used.

The encryption type value is:

```text
2 = AES-256-GCM
```

## Run

From the monorepo root:

```bash
npm install
npm run build
npm run start --workspace @moq-secure/example-basic
```

Expected output:

```text
hello from moq-secure
```

## Source

The example implementation is in:

```text
src/index.ts
```
