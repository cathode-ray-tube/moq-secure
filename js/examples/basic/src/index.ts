import {
  InMemoryKeyStore,
  decryptFrame,
  encryptFrame,
} from "moq-secure";

const keyStore = new InMemoryKeyStore();

const key = new Uint8Array(32);
crypto.getRandomValues(key);
keyStore.setKey(1, key);

const plaintext = new TextEncoder().encode("hello from moq-secure");

const frame = await encryptFrame(
  keyStore,
  new Uint8Array(32),
  1,
  0n,
  0,
  false,
  1,
  8,
  plaintext,
);

const encoded = frame.serialize();

const decoded = await decryptFrame(
  keyStore,
  new Uint8Array(32),
  { remaining: 0 },
  encoded,
);

console.log(new TextDecoder().decode(decoded));
