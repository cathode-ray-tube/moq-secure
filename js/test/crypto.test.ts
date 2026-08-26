import { describe, expect, it } from "vitest";
import {
  aeadDecrypt,
  aeadEncrypt,
  sha256Digest,
} from "../src/crypto.js";
import { deriveNonce12 } from "../src/nonce.js";

const key = Uint8Array.from(
  "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
    .match(/../g)!
    .map((x) => parseInt(x, 16)),
);

const hex = (value: string) =>
  Uint8Array.from(value.match(/../g)!.map((x) => parseInt(x, 16)));

describe("crypto", () => {
  it("computes SHA-256 digests", () => {
    expect(Buffer.from(sha256Digest(new Uint8Array())).toString("hex"))
      .toBe(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      );
  });

  it("encrypts and decrypts an empty plaintext", () => {
    const aad = hex(
      "4d4f5153010700000000000000000000010000000000000000000000",
    );

    const result = aeadEncrypt(key, 7, 0n, aad, new Uint8Array());

    expect(result.ciphertext).toEqual(new Uint8Array());
    expect(Buffer.from(result.tag).toString("hex"))
      .toBe("f29b1e0902a13f3d1ac744797f117606");

    expect(aeadDecrypt(
      key,
      7,
      0n,
      aad,
      result.ciphertext,
      result.tag,
    )).toEqual(new Uint8Array());
  });

  it("round-trips binary plaintext", () => {
    const aad = new Uint8Array([1, 2, 3]);
    const plaintext = hex("0001027f80feff");

    const result = aeadEncrypt(key, 7, 1n, aad, plaintext);

    expect(aeadDecrypt(
      key,
      7,
      1n,
      aad,
      result.ciphertext,
      result.tag,
    )).toEqual(plaintext);
  });

  it("rejects keys of the wrong length", () => {
    expect(() => aeadEncrypt(
      new Uint8Array(31),
      0,
      0n,
      new Uint8Array(),
      new Uint8Array(),
    )).toThrow("AEAD key must be 32 bytes");

    expect(() => aeadDecrypt(
  new Uint8Array(31),
  0,
  0n,
  new Uint8Array(),
  new Uint8Array(),
  new Uint8Array(16),
)).toThrowError(expect.objectContaining({
  code: "AeadAuthFailed",
}));

  it("rejects an invalid authentication tag", () => {
    const result = aeadEncrypt(
      key,
      1,
      2n,
      new Uint8Array([9]),
      new Uint8Array([1, 2, 3]),
    );

    result.tag[0] ^= 1;

    expect(() => aeadDecrypt(
  key,
  1,
  2n,
  new Uint8Array([9]),
  result.ciphertext,
  result.tag,
)).toThrowError(expect.objectContaining({
  code: "AeadAuthFailed",
}));

  it("derives the same nonce used by encryption", () => {
    expect(deriveNonce12(7, 42n)).toEqual(
      hex("3ea7d92eeec70d0a61fd1423"),
    );
  });
});
