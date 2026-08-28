# Basic JavaScript Example

This example demonstrates a basic encrypted `moq-secure` frame round trip:

1. Generate a 32-byte encryption key.
2. Store the key in an in-memory key store.
3. Encrypt plaintext into a `Frame`.
4. Serialize the frame to bytes.
5. Decrypt the serialized frame.
6. Print the recovered plaintext.

## Run

From the monorepo root:

```bash
npm install
npm run build
npm run start --workspace @moq-secure/example-basic
````

Expected output:
```text
hello from moq-secure
```

## Source

The example implementation is in `src/index.ts`

The example uses encryption without signatures. The nSigned value is 0, so the placeholder signing keys are not used.
