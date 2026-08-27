import { describe, expect, it } from "vitest";

import vectors from "../../test-vectors/frames.json";

import {
  aeadDecrypt,
  aeadEncrypt,
  sha256Digest,
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

const hex = (value: string): Uint8Array =>
  Uint8Array.from(
    value.match(/../g)?.map((part) => parseInt(part, 16)) ?? [],
  );

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

  it("matches the generated encrypted empty-frame vector", () => {
    const expected = frameVector("encrypted_unsigned_empty");
    const header = hex(expected.header);
    const payload = hex(expected.payload);

    const keyId = header[5];
    const ctr = readU64BE(header, 6);

    const result = aeadEncrypt(
      key,
      keyId,
      ctr,
      header,
      payload,
    );

    expect(result.ciphertext).toEqual(
      new Uint8Array(),
    );

    expect(result.tag).toEqual(hex(expected.tag));

    expect(
      aeadDecrypt(
        key,
        keyId,
        ctr,
        header,
        result.ciphertext,
        result.tag,
      ),
    ).toEqual(payload);
  });

  it("matches the generated encrypted binary-frame vector", () => {
    const expected = frameVector("encrypted_unsigned_binary");
    const header = hex(expected.header);
    const payload = hex(expected.payload);
    const serialized = hex(expected.frame);

    const keyId = header[5];
    const ctr = readU64BE(header, 6);

    const result = aeadEncrypt(
      key,
      keyId,
      ctr,
      header,
      payload,
    );

    expect(result.ciphertext).toEqual(
      serialized.slice(
        header.length,
        serialized.length - 16,
      ),
    );

    expect(result.tag).toEqual(hex(expected.tag));

    expect(
      aeadDecrypt(
        key,
        keyId,
        ctr,
        header,
        result.ciphertext,
        result.tag,
      ),
    ).toEqual(payload);
  });

  it("round-trips binary plaintext", () => {
    const aad = hex(
      "4d4f515301070000000000000000000000000001",
    );

    const plaintext = hex("0001027f80feff");

    const result = aeadEncrypt(
      key,
      7,
      1n,
      aad,
      plaintext,
    );

    expect(
      aeadDecrypt(
        key,
        7,
        1n,
        aad,
        result.ciphertext,
        result.tag,
      ),
    ).toEqual(plaintext);
  });

  it("rejects keys of the wrong length", () => {
    expect(() =>
      aeadEncrypt(
        new Uint8Array(31),
        0,
        0n,
        new Uint8Array(),
        new Uint8Array(),
      ),
    ).toThrow("AEAD key must be 32 bytes");

    expect(() =>
      aeadDecrypt(
        new Uint8Array(31),
        0,
        0n,
        new Uint8Array(),
        new Uint8Array(),
        new Uint8Array(16),
      ),
    ).toThrowError(
      expect.objectContaining({
        code: "AeadAuthFailed",
      }),
    );
  });

  it("rejects an invalid authentication tag", () => {
    const result = aeadEncrypt(
      key,
      1,
      2n,
      new Uint8Array([9]),
      new Uint8Array([1, 2, 3]),
    );

    result.tag[0] ^= 1;

    expect(() =>
      aeadDecrypt(
        key,
        1,
        2n,
        new Uint8Array([9]),
        result.ciphertext,
        result.tag,
      ),
    ).toThrowError(
      expect.objectContaining({
        code: "AeadAuthFailed",
      }),
    );
  });

  it("derives the expected nonce", () => {
    expect(deriveNonce12(7, 42n)).toEqual(
      hex("3ea7d92eeec70d0a61fd1423"),
    );
  });
});
