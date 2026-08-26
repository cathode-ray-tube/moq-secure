import { describe, expect, it } from "vitest";
import {
  aeadDecrypt,
  aeadEncrypt,
  deriveNonce12,
  MoqSecureError,
} from "../src/index.js";

const key = Uint8Array.from(
  Array.from({ length: 32 }, (_, i) => i),
);

const aad = new Uint8Array([1, 2, 3, 4]);
const plaintext = new Uint8Array([10, 20, 30, 40]);

describe("nonce derivation", () => {
  it("is deterministic", () => {
    const a = deriveNonce12(7, 42n);
    const b = deriveNonce12(7, 42n);

    expect([...a]).toEqual([...b]);
    expect(a).toHaveLength(12);
  });

  it("changes when key ID changes", () => {
    expect([...deriveNonce12(7, 42n)])
      .not.toEqual([...deriveNonce12(8, 42n)]);
  });

  it("changes when counter changes", () => {
    expect([...deriveNonce12(7, 42n)])
      .not.toEqual([...deriveNonce12(7, 43n)]);
  });

  it("rejects invalid inputs", () => {
    expect(() => deriveNonce12(-1, 0n)).toThrow(RangeError);
    expect(() => deriveNonce12(256, 0n)).toThrow(RangeError);
    expect(() => deriveNonce12(0, -1n)).toThrow(RangeError);
    expect(() => deriveNonce12(0, 0x1_0000_0000_0000_0000n))
      .toThrow(RangeError);
  });
});

describe("ChaCha20-Poly1305", () => {
  it("round-trips", () => {
    const { ciphertext, tag } = aeadEncrypt(
      key,
      7,
      42n,
      aad,
      plaintext,
    );

    expect(ciphertext).toHaveLength(plaintext.length);
    expect(tag).toHaveLength(16);

    const result = aeadDecrypt(
      key,
      7,
      42n,
      aad,
      ciphertext,
      tag,
    );

    expect([...result]).toEqual([...plaintext]);
  });

  it("rejects modified ciphertext", () => {
    const result = aeadEncrypt(key, 7, 42n, aad, plaintext);
    result.ciphertext[0] ^= 1;

    expect(() =>
      aeadDecrypt(key, 7, 42n, aad, result.ciphertext, result.tag),
    ).toThrowError(MoqSecureError);

    try {
      aeadDecrypt(key, 7, 42n, aad, result.ciphertext, result.tag);
    } catch (error) {
      expect((error as MoqSecureError).code).toBe("AeadAuthFailed");
    }
  });

  it("rejects modified AAD", () => {
    const result = aeadEncrypt(key, 7, 42n, aad, plaintext);
    const modifiedAad = aad.slice();
    modifiedAad[0] ^= 1;

    expect(() =>
      aeadDecrypt(key, 7, 42n, modifiedAad, result.ciphertext, result.tag),
    ).toThrowError(MoqSecureError);
  });

  it("rejects wrong key length", () => {
    expect(() =>
      aeadEncrypt(new Uint8Array(31), 7, 0n, aad, plaintext),
    ).toThrow(RangeError);

    expect(() =>
      aeadDecrypt(
        new Uint8Array(31),
        7,
        0n,
        aad,
        plaintext,
        new Uint8Array(16),
      ),
    ).toThrowError(MoqSecureError);
  });
});
