import { describe, expect, it } from "vitest";
import { privateKeyFromSeed, getPublicKey } from "@noble/ed25519";

import {
  Frame,
  InMemoryKeyStore,
  MAGIC,
  MoqSecureError,
  WireHeader,
  decryptFrame,
  encryptFrame,
} from "../src/index.js";

const key = new Uint8Array(
  Array.from({ length: 32 }, (_, i) => i),
);

const seed = new Uint8Array(
  Array.from({ length: 32 }, (_, i) => 31 - i),
);

async function setup() {
  const store = new InMemoryKeyStore();
  store.setKey(7, key);

  const privateKey = await privateKeyFromSeed(seed);
  const publicKey = await getPublicKey(privateKey);

  return { store, privateKey, publicKey };
}

describe("WireHeader", () => {
  it("encodes to the specified 28-byte layout", () => {
    const header = new WireHeader(
      MAGIC,
      1,
      7,
      0x0102_0304_0506_0708n,
      3,
      1,
      1,
      9,
    );

    expect(header.encode()).toHaveLength(28);
    expect([...header.encode().slice(0, 6)])
      .toEqual([0x4d, 0x4f, 0x51, 0x53, 1, 7]);
  });

  it("rejects invalid flags", () => {
    expect(() => new WireHeader(
      MAGIC, 1, 0, 0n, 1, 2, 0, 0,
    ).validate()).toThrowError(MoqSecureError);

    expect(() => new WireHeader(
      MAGIC, 1, 0, 0n, 1, 0, 2, 0,
    ).validate()).toThrowError(MoqSecureError);
  });
});

describe("frames", () => {
  it("round-trips encrypted unsigned frames", async () => {
    const { store, privateKey, publicKey } = await setup();
    const plaintext = new Uint8Array([1, 2, 3, 4]);

    const frame = await encryptFrame(
      store, privateKey, 7, 10n, 0, false, 1, 3, plaintext,
    );

    const parsed = Frame.parse(frame.serialize());
    const lease = { remaining: 0 };

    await expect(
      parsed.decodePlaintext(store, publicKey, lease),
    ).resolves.toEqual(plaintext);
  });

  it("round-trips cleartext unsigned frames", async () => {
    const { store, privateKey, publicKey } = await setup();
    const plaintext = new TextEncoder().encode("clear");

    const frame = await encryptFrame(
      store, privateKey, 7, 11n, 0, false, 0, 2, plaintext,
    );

    const lease = { remaining: 0 };

    await expect(
      decryptFrame(store, publicKey, lease, frame.serialize()),
    ).resolves.toEqual(plaintext);
  });

  it("verifies signed frames and initializes the lease", async () => {
    const { store, privateKey, publicKey } = await setup();

    const frame = await encryptFrame(
      store, privateKey, 7, 12n, 3, true, 1, 0, new Uint8Array([8]),
    );

    const lease = { remaining: 0 };

    await expect(
      decryptFrame(store, publicKey, lease, frame.serialize()),
    ).resolves.toEqual(new Uint8Array([8]));

    expect(lease.remaining).toBe(3);
  });

  it("rejects a tampered signature", async () => {
    const { store, privateKey, publicKey } = await setup();

    const frame = await encryptFrame(
      store, privateKey, 7, 13n, 1, true, 1, 0, new Uint8Array([8]),
    );

    const encoded = frame.serialize();
    encoded[encoded.length - 1] ^= 1;

    await expect(
      decryptFrame(store, publicKey, { remaining: 0 }, encoded),
    ).rejects.toMatchObject({ code: "InvalidSignature" });
  });

  it("rejects unsigned frames after the lease expires", async () => {
    const { store, privateKey, publicKey } = await setup();

    const signed = await encryptFrame(
      store, privateKey, 7, 14n, 2, true, 1, 0, new Uint8Array([1]),
    );

    const unsigned = await encryptFrame(
      store, privateKey, 7, 15n, 2, false, 1, 0, new Uint8Array([2]),
    );

    const lease = { remaining: 0 };
    await decryptFrame(store, publicKey, lease, signed.serialize());

    await decryptFrame(store, publicKey, lease, unsigned.serialize());
    await decryptFrame(store, publicKey, lease, unsigned.serialize());

    await expect(
      decryptFrame(store, publicKey, lease, unsigned.serialize()),
    ).rejects.toMatchObject({ code: "InvalidSignature" });
  });

  it("rejects truncated and malformed frames", () => {
    expect(() => Frame.parse(new Uint8Array(27)))
      .toThrowError(MoqSecureError);

    const header = new Uint8Array(28);
    header.set([0x4d, 0x4f, 0x51, 0x53]);

    expect(() => Frame.parse(header))
      .toThrowError(MoqSecureError);
  });
});
