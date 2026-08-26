import { createHash } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import * as ed25519 from "@noble/ed25519";

import {
  InMemoryKeyStore,
  deriveNonce12,
  encryptFrame,
  type Frame,
} from "../js/src/index.js";

// Configure SHA-512 for @noble/ed25519 using Node.js built-in crypto.
ed25519.hashes.sha512 = (...messages: Uint8Array[]) => {
  const hash = createHash("sha512");

  for (const message of messages) {
    hash.update(message);
  }

  return new Uint8Array(hash.digest());
};

function hex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function bytes(...values: number[]): Uint8Array {
  return new Uint8Array(values);
}

function utf8(value: string): Uint8Array {
  return new TextEncoder().encode(value);
}

function frameRecord(
  name: string,
  frame: Frame,
  plaintext: Uint8Array,
) {
  return {
    name,
    plaintext: hex(plaintext),
    frame: hex(frame.serialize()),
    header: hex(frame.header.encode()),
    payload: hex(frame.payload),
    tag: hex(frame.tag),
    signature: frame.signature ? hex(frame.signature) : null,
    lease: frame.header.nSigned,
  };
}

const aeadKey = Uint8Array.from(
  Array.from({ length: 32 }, (_, i) => i),
);

const ed25519Seed = Uint8Array.from(
  Array.from({ length: 32 }, (_, i) => 31 - i),
);

const privateKey = ed25519Seed;

const keyStore = new InMemoryKeyStore();
keyStore.setKey(7, aeadKey);

const nonceInputs = [
  { keyId: 0, ctr: 0n },
  { keyId: 1, ctr: 1n },
  { keyId: 7, ctr: 42n },
  { keyId: 255, ctr: 0xffff_ffff_ffff_ffffn },
];

const nonceVectors = nonceInputs.map(({ keyId, ctr }) => ({
  keyId,
  ctr: ctr.toString(),
  nonce: hex(deriveNonce12(keyId, ctr)),
}));

const cases: Array<{
  name: string;
  keyId: number;
  ctr: bigint;
  nSigned: number;
  maybeSign: boolean;
  encrypted: number;
  padLen: number;
  plaintext: Uint8Array;
}> = [
  {
    name: "encrypted_unsigned_empty",
    keyId: 7,
    ctr: 0n,
    nSigned: 0,
    maybeSign: false,
    encrypted: 1,
    padLen: 0,
    plaintext: new Uint8Array(),
  },
  {
    name: "encrypted_unsigned_binary",
    keyId: 7,
    ctr: 1n,
    nSigned: 0,
    maybeSign: false,
    encrypted: 1,
    padLen: 3,
    plaintext: bytes(0, 1, 2, 127, 128, 254, 255),
  },
  {
    name: "encrypted_signed",
    keyId: 7,
    ctr: 2n,
    nSigned: 3,
    maybeSign: true,
    encrypted: 1,
    padLen: 5,
    plaintext: utf8("signed encrypted media"),
  },
  {
    name: "cleartext_unsigned",
    keyId: 7,
    ctr: 3n,
    nSigned: 0,
    maybeSign: false,
    encrypted: 0,
    padLen: 2,
    plaintext: utf8("cleartext"),
  },
  {
    name: "cleartext_signed",
    keyId: 7,
    ctr: 4n,
    nSigned: 2,
    maybeSign: true,
    encrypted: 0,
    padLen: 1,
    plaintext: bytes(0xde, 0xad, 0xbe, 0xef),
  },
];

const frames: ReturnType<typeof frameRecord>[] = [];

for (const testCase of cases) {
  const frame = await encryptFrame(
    keyStore,
    privateKey,
    testCase.keyId,
    testCase.ctr,
    testCase.nSigned,
    testCase.maybeSign,
    testCase.encrypted,
    testCase.padLen,
    testCase.plaintext,
  );

  frames.push(
    frameRecord(testCase.name, frame, testCase.plaintext),
  );
}

const output = {
  version: 1,
  aeadKey: hex(aeadKey),
  ed25519Seed: hex(ed25519Seed),
  nonceVectors,
  frames,
};

const outputPath = join(
  fileURLToPath(new URL(".", import.meta.url)),
  "frames.json",
);

await writeFile(
  outputPath,
  `${JSON.stringify(output, null, 2)}\n`,
);

console.log(`wrote ${outputPath}`);
