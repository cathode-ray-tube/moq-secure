import { describe, expect, it } from "vitest";

import {
  InMemoryKeyStore,
} from "../src/keys.js";

import {
  prependZeroPadding,
  removeZeroPadding,
} from "../src/padding.js";

const key = Uint8Array.from({ length: 32 }, (_, index) => index);
const hexKey = Buffer.from(key).toString("hex");
const base64Key = Buffer.from(key).toString("base64");
const base64UrlKey = base64Key
  .replace(/\+/g, "-")
  .replace(/\//g, "_")
  .replace(/=+$/, "");

describe("InMemoryKeyStore", () => {
  it("stores a defensive copy", () => {
    const store = new InMemoryKeyStore();
    const input = key.slice();

    store.setKey(0, input);
    input[0] = 255;

    expect(store.aeadKey(0)?.[0]).toBe(0);
  });

  it.each([0, 1, 255])(
    "accepts key id %s",
    (keyId) => {
      const store = new InMemoryKeyStore();

      store.setKey(keyId, key);

      expect(store.aeadKey(keyId)).toEqual(key);
    },
  );

  it.each([-1, 256, 1.5, NaN, Infinity])(
    "rejects invalid key id %s",
    (keyId) => {
      const store = new InMemoryKeyStore();

      expect(() => store.setKey(keyId, key))
        .toThrowError(
          expect.objectContaining({
            code: "KeyIdInvalid",
          }),
        );
    },
  );

  it("rejects keys with the wrong length", () => {
    expect(() =>
      new InMemoryKeyStore().setKey(
        1,
        new Uint8Array(31),
      ),
    ).toThrowError(
      expect.objectContaining({
        code: "KeyWrongLength",
      }),
    );
  });

  it("decodes hexadecimal keys", () => {
    const store = new InMemoryKeyStore();

    store.setKeyEncoded(1, hexKey.toUpperCase());

    expect(store.aeadKey(1)).toEqual(key);
  });

  it("decodes standard and URL-safe base64 keys", () => {
    const standard = new InMemoryKeyStore();
    standard.setKeyEncoded(1, base64Key);

    expect(standard.aeadKey(1)).toEqual(key);

    const urlSafe = new InMemoryKeyStore();
    urlSafe.setKeyEncoded(1, base64UrlKey);

    expect(urlSafe.aeadKey(1)).toEqual(key);
  });

  it("trims encoded keys", () => {
    const store = new InMemoryKeyStore();

    store.setKeyEncoded(1, ` \n${hexKey}\t `);

    expect(store.aeadKey(1)).toEqual(key);
  });

  it("rejects malformed or incorrectly sized encoded keys", () => {
    const store = new InMemoryKeyStore();

    expect(() => store.setKeyEncoded(0, "xyz"))
      .toThrowError(
        expect.objectContaining({
          code: "KeyWrongLength",
        }),
      );

    expect(() => store.setKeyEncoded(0, "00".repeat(31)))
      .toThrowError(
        expect.objectContaining({
          code: "KeyWrongLength",
        }),
      );
  });

  it("does not expose missing keys", () => {
    expect(
      new InMemoryKeyStore().aeadKey(9),
    ).toBeUndefined();
  });
});

describe("padding", () => {
  it("prepends zero bytes", () => {
    expect(
      prependZeroPadding(
        Uint8Array.from([1, 2, 3]),
        2,
      ),
    ).toEqual(
      Uint8Array.from([0, 0, 1, 2, 3]),
    );
  });

  it("removes padding", () => {
    expect(
      removeZeroPadding(
        Uint8Array.from([0, 0, 1, 2, 3]),
        2,
      ),
    ).toEqual(
      Uint8Array.from([1, 2, 3]),
    );
  });

  it("returns copies when padding is zero", () => {
    const input = Uint8Array.from([1, 2]);
    const output = prependZeroPadding(input, 0);

    expect(output).toEqual(input);
    expect(output).not.toBe(input);
  });

  it.each([
    -1,
    1.1,
    Number.MAX_SAFE_INTEGER + 1,
  ])(
    "rejects invalid padding length %s",
    (length) => {
      expect(() =>
        prependZeroPadding(
          new Uint8Array(),
          length,
        ),
      ).toThrow(RangeError);
    },
  );

  it("rejects padding longer than plaintext", () => {
    expect(() =>
      removeZeroPadding(
        new Uint8Array(2),
        3,
      ),
    ).toThrow("plaintext is shorter than padLength");
  });
});

