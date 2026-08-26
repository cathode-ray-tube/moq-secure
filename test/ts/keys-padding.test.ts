import { describe, expect, it } from "vitest";
import {
  InMemoryKeyStore,
  KeyStoreError,
  prependZeroPadding,
  removeZeroPadding,
} from "../src/index.js";

describe("InMemoryKeyStore", () => {
  it("copies keys when storing them", () => {
    const store = new InMemoryKeyStore();
    const key = new Uint8Array(32).fill(7);

    store.setKey(3, key);
    key[0] = 99;

    expect(store.aeadKey(3)?.[0]).toBe(7);
  });

  it("rejects keys with the wrong length", () => {
    const store = new InMemoryKeyStore();

    expect(() => store.setKey(0, new Uint8Array(31)))
      .toThrowError(KeyStoreError);
  });

  it("accepts hexadecimal keys", () => {
    const store = new InMemoryKeyStore();
    const encoded = "00".repeat(32);

    store.setKeyEncoded(4, encoded);

    expect(store.aeadKey(4)).toEqual(new Uint8Array(32));
  });

  it("accepts standard and URL-safe base64", () => {
    const store = new InMemoryKeyStore();
    const key = new Uint8Array(32).fill(0xff);

    // This is the standard base64 representation of 32 bytes of 0xff.
    store.setKeyEncoded(1, "/////////////////////////////////////////w==");
    expect(store.aeadKey(1)).toEqual(key);

    store.setKeyEncoded(2, "_________________________________________w");
    expect(store.aeadKey(2)).toEqual(key);
  });

  it("rejects invalid key IDs", () => {
    const store = new InMemoryKeyStore();

    expect(() => store.setKey(-1, new Uint8Array(32)))
      .toThrowError(KeyStoreError);

    expect(() => store.setKey(256, new Uint8Array(32)))
      .toThrowError(KeyStoreError);
  });
});

describe("padding", () => {
  it("prepends zero bytes", () => {
    expect(prependZeroPadding(new Uint8Array([1, 2]), 3))
      .toEqual(new Uint8Array([0, 0, 0, 1, 2]));
  });

  it("removes exactly the requested prefix", () => {
    expect(removeZeroPadding(new Uint8Array([0, 0, 1, 2]), 2))
      .toEqual(new Uint8Array([1, 2]));
  });

  it("does not require padding bytes to be zero", () => {
    expect(removeZeroPadding(new Uint8Array([9, 8, 1]), 2))
      .toEqual(new Uint8Array([1]));
  });

  it("rejects excessive padding length", () => {
    expect(() => removeZeroPadding(new Uint8Array([1]), 2))
      .toThrow(RangeError);
  });
});
