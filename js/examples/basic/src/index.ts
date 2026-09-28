import {
  InMemoryKeyStore,
  decryptFrame,
  encryptFrame,
} from "moq-secure";

const keyStore = new InMemoryKeyStore();

const key = new Uint8Array(32); // 32 bytes = AES-256
crypto.getRandomValues(key);
keyStore.setKey(1, key);

const plaintext = new TextEncoder().encode("hello from moq-secure");

const frame = await encryptFrame(
  keyStore,
  new Uint8Array(32), // unused because maybeSign is false
  1,                  // key ID
  0n,                 // counter
  0,                  // nSigned
  false,              // maybeSign
  2,                  // AES-256-GCM
  8,                  // padding length
  plaintext,
);

const encoded = frame.serialize();

const decoded = await decryptFrame(
  keyStore,
  new Uint8Array(32), // unused because the frame is unsigned
  { remaining: 0 },
  encoded,
);

console.log(new TextDecoder().decode(decoded));
