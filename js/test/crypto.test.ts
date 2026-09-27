import { describe, expect, it } from "vitest";

import vectors from "../../test-vectors/frames.json";

import {
  aeadDecrypt,
  aeadEncrypt,
  sha256Digest,
  type AeadAlgorithm,
} from "../src/crypto.js";
import { deriveNonce12 } from "../src/nonce.js";

type FrameVector = {
  name: string;
  plaintext: string;
  padLen: number;
  frame: string;
  header: string;
  payload: string;
  tag: string;
  signature: string | null;
  lease: number;
};

type TestVectors = {
  aeadKey: string;
  frames: FrameVector[];
};

const testVectors = vectors as TestVectors;

const algorithms: AeadAlgorithm[] = [
  "CHACHA20-POLY1305",
  "AES-256-GCM",
];

const hex = (value: string): Uint8Array =>
  Uint8Array.from(
    value.match(/../g)?.map((part) => parseInt(part, 16)) ?? [],
  );

const encodeUint32BE = (value: number): Uint8Array =>
  Uint8Array.from([
    (value >>> 24) & 0xff,
    (value >>> 16) & 0xff,
    (value >>> 8) & 0xff,
    value & 0xff,
  ]);

const prependZeroPadding = (
  plaintext: Uint8Array,
  padLen: number,
): Uint8Array => {
  const padded = new Uint8Array(padLen + plaintext.length);
  padded.set(plaintext, padLen);
  return padded;
};

const concat = (...values: Uint8Array[]): Uint8Array => {
  const result = new Uint8Array(
    values.reduce((length, value) => length + value.length, 0),
  );

  let offset = 0;

  for (const value of values) {
    result.set(value, offset);
    offset += value.length;
  }

  return result;
};

function readU64BE(bytes: Uint8Array, offset: number): bigint {
  let value = 0n;

  for (let index = 0; index < 8; index++) {
    value = (value << 8n) | BigInt(bytes[offset + index]);
  }

  return value;
}

function frameVector(name: string): FrameVector {
  const result = testVectors.frames.find((frame) => frame.name === name);

  if (!result) {
    throw new Error(`Missing frame vector: ${name}`);
  }

  return result;
}

const key = hex(testVectors.aeadKey);

describe("crypto", () => {
  it("computes SHA-256 digests", () => {
    expect(
      Buffer.from(sha256Digest(new Uint8Array())).toString("hex"),
    ).toBe(
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
  });

  it("matches the generated encrypted empty-frame vector", async () => {
    const expected = frameVector("encrypted_unsigned_empty");
    const header = hex(expected.header);
    const serialized = hex(expected.frame);
    const tag = hex(expected.tag);

    const plaintext = hex(expected.plaintext);
    const paddedPlaintext = concat(
      encodeUint32BE(expected.padLen),
      prependZeroPadding(plaintext, expected.padLen),
    );

    const keyId = header[5];
    const ctr = readU64BE(header, 6);

    const result = await aeadEncrypt(
      "CHACHA20-POLY1305",
      key,
      keyId,
      ctr,
      header,
      paddedPlaintext,
    );

    expect(result.ciphertext).toEqual(
      serialized.slice(
        header.length,
        serialized.length - tag.length,
      ),
    );

    expect(result.tag).toEqual(tag);

    await expect(
      aeadDecrypt(
        "CHACHA20-POLY1305",
        key,
        keyId,
        ctr,
        header,
        result.ciphertext,
        result.tag,
      ),
    ).resolves.toEqual(paddedPlaintext);
  });

  it("matches the generated encrypted binary-frame vector", async () => {
    const expected = frameVector("encrypted_unsigned_binary");
    const header = hex(expected.header);
    const serialized = hex(expected.frame);
    const tag = hex(expected.tag);

    const paddedPlaintext = Uint8Array.from([
      0x00, 0x00, 0x00, 0x03,
      0x00, 0x00, 0x00,
      0x00, 0x01, 0x02, 0x7f, 0x80, 0xfe, 0xff,
    ]);

    const keyId = header[5];
    const ctr = readU64BE(header, 6);

    const result = await aeadEncrypt(
      "CHACHA20-POLY1305",
      key,
      keyId,
      ctr,
      header,
      paddedPlaintext,
    );

    expect(result.ciphertext).toEqual(
      serialized.slice(
        header.length,
        serialized.length - tag.length,
      ),
    );

    expect(result.tag).toEqual(tag);

    await expect(
      aeadDecrypt(
        "CHACHA20-POLY1305",
        key,
        keyId,
        ctr,
        header,
        result.ciphertext,
        result.tag,
      ),
    ).resolves.toEqual(paddedPlaintext);
  });

  it.each(algorithms)(
    "round-trips binary plaintext with %s",
    async (algorithm) => {
      const aad = hex(
        "4d4f515301070000000000000000000000000001",
      );

      const plaintext = hex("0001027f80feff");

      const result = await aeadEncrypt(
        algorithm,
        key,
        7,
        1n,
        aad,
        plaintext,
      );

      await expect(
        aeadDecrypt(
          algorithm,
          key,
          7,
          1n,
          aad,
          result.ciphertext,
          result.tag,
        ),
      ).resolves.toEqual(plaintext);
    },
  );

  it("rejects keys of the wrong length", async () => {
    await expect(
      aeadEncrypt(
        "AES-256-GCM",
        new Uint8Array(31),
        0,
        0n,
        new Uint8Array(),
        new Uint8Array(),
      ),
    ).rejects.toThrow("AEAD key must be 32 bytes");

    await expect(
      aeadDecrypt(
        "AES-256-GCM",
        new Uint8Array(31),
        0,
        0n,
        new Uint8Array(),
        new Uint8Array(),
        new Uint8Array(16),
      ),
    ).rejects.toThrowError(
      expect.objectContaining({
        code: "AeadAuthFailed",
      }),
    );
  });

  it.each(algorithms)(
    "rejects an invalid authentication tag with %s",
    async (algorithm) => {
      const result = await aeadEncrypt(
        algorithm,
        key,
        1,
        2n,
        new Uint8Array([9]),
        new Uint8Array([1, 2, 3]),
      );

      result.tag[0] ^= 1;

      await expect(
        aeadDecrypt(
          algorithm,
          key,
          1,
          2n,
          new Uint8Array([9]),
          result.ciphertext,
          result.tag,
        ),
      ).rejects.toThrowError(
        expect.objectContaining({
          code: "AeadAuthFailed",
        }),
      );
    },
  );

  it.each(algorithms)(
    "rejects modified AAD with %s",
    async (algorithm) => {
      const result = await aeadEncrypt(
        algorithm,
        key,
        7,
        42n,
        new Uint8Array([1, 2, 3]),
        new Uint8Array([9, 8, 7]),
      );

      await expect(
        aeadDecrypt(
          algorithm,
          key,
          7,
          42n,
          new Uint8Array([1, 2, 4]),
          result.ciphertext,
          result.tag,
        ),
      ).rejects.toThrowError(
        expect.objectContaining({
          code: "AeadAuthFailed",
        }),
      );
    },
  );

  it("derives the expected nonce", () => {
    expect(deriveNonce12(7, 42n)).toEqual(
      hex("6e6f6e07000000000000002a"),
    );
  });
});
