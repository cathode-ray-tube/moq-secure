import { describe, expect, it } from "vitest";
import * as ed25519 from "@noble/ed25519";
import {
  Frame,
  WireHeader,
  decryptFrame,
  encryptFrame,
} from "../src/wire.js";
import { InMemoryKeyStore } from "../src/keys.js";
import { MoqSecureError } from "../src/errors.js";
import { MAGIC, VERSION } from "../src/constants.js";

const hex = (value: string) =>
  Uint8Array.from(value.match(/../g)!.map((x) => parseInt(x, 16)));

const key = hex(
  "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
);

// Correct 32-byte seed from the supplied vector.
const signingSeed = hex(
  "1f1e1d1c1b1a191817161514131211100f0e0d0c0b0a09080706050403020100",
);

describe("WireHeader", () => {
  it("encodes and parses a header", () => {
    const header = new WireHeader(
      MAGIC,
      VERSION,
      7,
      42n,
      3,
      1,
      1,
      5,
    );

    const parsed = Frame.parse(
      new Uint8Array([...header.encode(), ...new Uint8Array(16)]),
    ).header;

    expect(parsed.magic).toEqual(MAGIC);
    expect(parsed.version).toBe(1);
    expect(parsed.keyId).toBe(7);
    expect(parsed.ctr).toBe(42n);
    expect(parsed.nSigned).toBe(3);
    expect(parsed.sigFlag).toBe(1);
    expect(parsed.encrypted).toBe(1);
    expect(parsed.padLen).toBe(5);
  });

  it("rejects invalid headers", () => {
    const header = new WireHeader(
      new Uint8Array([0, 0, 0, 0]),
      1,
      0,
      0n,
      0,
      0,
      0,
      0,
    );

    expect(() => header.validate()).toThrowError(
      expect.objectContaining({ code: "InvalidMagic" }),
    );
  });

  it("rejects signing when nSigned is zero", () => {
    const header = new WireHeader(MAGIC, 1, 0, 0n, 0, 1, 0, 0);

    expect(() => header.validate()).toThrowError(
      expect.objectContaining({ code: "SigningMismatch" }),
    );
  });
});

describe("Frame", () => {
  it("round-trips encrypted unsigned data", async () => {
    const keys = new InMemoryKeyStore();
    keys.setKey(7, key);

    const frame = await encryptFrame(
      keys,
      signingSeed,
      7,
      0n,
      0,
      false,
      1,
      0,
      new Uint8Array(),
    );

    const decoded = await decryptFrame(
      keys,
      await ed25519.getPublicKeyAsync(signingSeed),
      { remaining: 0 },
      frame.serialize(),
    );

    expect(decoded).toEqual(new Uint8Array());
    expect(frame.payload).toEqual(new Uint8Array());
    expect(frame.tag).toEqual(
      hex("f29b1e0902a13f3d1ac744797f117606"),
    );
  });

  it("round-trips cleartext with zero padding", async () => {
    const keys = new InMemoryKeyStore();
    const plaintext = hex("636c65617274657874");

    const frame = await encryptFrame(
      keys,
      signingSeed,
      7,
      3n,
      0,
      false,
      0,
      2,
      plaintext,
    );

    expect(frame.payload).toEqual(hex("0000636c65617274657874"));

    expect(await decryptFrame(
      keys,
      new Uint8Array(),
      { remaining: 0 },
      frame.serialize(),
    )).toEqual(plaintext);
  });

  it("signs and verifies a frame while updating the lease", async () => {
    const keys = new InMemoryKeyStore();
    keys.setKey(7, key);

    const frame = await encryptFrame(
      keys,
      signingSeed,
      7,
      2n,
      3,
      true,
      1,
      5,
      hex("7369676e656420656e63727970746564206d65646961"),
    );

    const lease = { remaining: 0 };
    const publicKey = await ed25519.getPublicKeyAsync(signingSeed);

    expect(await decryptFrame(
      keys,
      publicKey,
      lease,
      frame.serialize(),
    )).toEqual(hex("7369676e656420656e63727970746564206d65646961"));

    expect(lease.remaining).toBe(3);
  });

  it("rejects tampered ciphertext", async () => {
    const keys = new InMemoryKeyStore();
    keys.setKey(7, key);

    const frame = await encryptFrame(
      keys,
      signingSeed,
      7,
      0n,
      0,
      false,
      1,
      0,
      bytes(1, 2, 3),
    );

    const encoded = frame.serialize();
    encoded[encoded.length - 1] ^= 1;

    await expect(decryptFrame(
      keys,
      new Uint8Array(),
      { remaining: 0 },
      encoded,
    )).rejects.toThrowError(
      expect.objectContaining({ code: "AeadAuthFailed" }),
    );
  });

  it("rejects unknown encryption keys", async () => {
    const frame = await encryptFrame(
      new InMemoryKeyStore(),
      signingSeed,
      7,
      0n,
      0,
      false,
      0,
      0,
      bytes(1),
    );

    const header = new WireHeader(MAGIC, 1, 7, 0n, 0, 0, 1, 0);
    const encrypted = new Frame({
      header,
      payload: frame.payload,
      tag: new Uint8Array(16),
    });

    await expect(encrypted.decodePlaintext(
      new InMemoryKeyStore(),
      new Uint8Array(),
      { remaining: 0 },
    )).rejects.toThrowError(
      expect.objectContaining({ code: "InvalidKeyId" }),
    );
  });

  it("rejects truncated and invalid frames", () => {
    expect(() => Frame.parse(new Uint8Array(27)))
      .toThrowError(expect.objectContaining({ code: "TruncatedFrame" }));

    const invalid = new Uint8Array(28);
    invalid.set(MAGIC);
    invalid[4] = 99;

    expect(() => Frame.parse(invalid))
      .toThrowError(expect.objectContaining({ code: "UnsupportedVersion" }));
  });
});

function bytes(...values: number[]) {
  return new Uint8Array(values);
}
