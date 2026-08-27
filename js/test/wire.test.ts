import { describe, expect, it } from "vitest";
import * as ed25519 from "@noble/ed25519";

import vectors from "../../test-vectors/frames.json";

import {
  Frame,
  WireHeader,
  decryptFrame,
  encryptFrame,
} from "../src/wire.js";
import { InMemoryKeyStore } from "../src/keys.js";
import { MAGIC, VERSION } from "../src/constants.js";

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
  version: number;
  aeadKey: string;
  ed25519Seed: string;
  nonceVectors: Array<{
    keyId: number;
    ctr: string;
    nonce: string;
  }>;
  frames: FrameVector[];
};

const testVectors = vectors as TestVectors;

const hex = (value: string): Uint8Array =>
  Uint8Array.from(
    value.match(/../g)?.map((part) => parseInt(part, 16)) ?? [],
  );

const bytes = (...values: number[]) =>
  new Uint8Array(values);

function readU64BE(
  value: Uint8Array,
  offset: number,
): bigint {
  let result = 0n;

  for (let index = 0; index < 8; index++) {
    result = (result << 8n) |
      BigInt(value[offset + index]);
  }

  return result;
}

function vector(name: string): FrameVector {
  const result = testVectors.frames.find(
    (frame) => frame.name === name,
  );

  if (!result) {
    throw new Error(`Missing frame vector: ${name}`);
  }

  return result;
}

function headerFields(headerHex: string) {
  const header = hex(headerHex);

  return {
    bytes: header,
    keyId: header[5],
    ctr: readU64BE(header, 6),
    nSigned: header[14],
    sigFlag: header[15],
    encrypted: header[16],
  };
}

function storeWithKey() {
  const store = new InMemoryKeyStore();
  store.setKey(7, hex(testVectors.aeadKey));
  return store;
}

describe("WireHeader", () => {
  it("encodes and parses the new 17-byte header", () => {
    const header = new WireHeader(
      MAGIC,
      VERSION,
      7,
      42n,
      3,
      0,
      1,
    );

    expect(header.encode()).toHaveLength(17);

    const encoded = new Uint8Array([
      ...header.encode(),

      // pad_len = 0
      0,
      0,
      0,
      0,

      // tag
      ...new Uint8Array(16),
    ]);

    const parsed = Frame.parse(encoded).header;

    expect(parsed.magic).toEqual(MAGIC);
    expect(parsed.version).toBe(VERSION);
    expect(parsed.keyId).toBe(7);
    expect(parsed.ctr).toBe(42n);
    expect(parsed.nSigned).toBe(3);
    expect(parsed.sigFlag).toBe(0);
    expect(parsed.encrypted).toBe(1);
  });

  it("does not include pad_len in the header", () => {
    const header = new WireHeader(
      MAGIC,
      VERSION,
      7,
      42n,
      3,
      0,
      1,
    );

    expect(header.encode()).toHaveLength(17);
  });

  it("rejects invalid magic", () => {
    const header = new WireHeader(
      new Uint8Array([0, 0, 0, 0]),
      VERSION,
      0,
      0n,
      0,
      0,
      0,
    );

    expect(() => header.validate()).toThrowError(
      expect.objectContaining({
        code: "InvalidMagic",
      }),
    );
  });

  it("rejects signing when nSigned is zero", () => {
    const header = new WireHeader(
      MAGIC,
      VERSION,
      0,
      0n,
      0,
      1,
      0,
    );

    expect(() => header.validate()).toThrowError(
      expect.objectContaining({
        code: "SigningMismatch",
      }),
    );
  });
});

describe("generated frame vectors", () => {
  it.each(testVectors.frames)(
    "$name has the expected serialized components",
    async (expected) => {
      const fields = headerFields(expected.header);

      const frame = await encryptFrame(
        storeWithKey(),
        hex(testVectors.ed25519Seed),
        fields.keyId,
        fields.ctr,
        fields.nSigned,
        fields.sigFlag === 1,
        fields.encrypted,
        expected.padLen,
        hex(expected.plaintext),
      );

      expect(frame.header.encode()).toEqual(
        fields.bytes,
      );

      expect(frame.payload).toEqual(
        hex(expected.payload),
      );

      expect(frame.tag).toEqual(
        hex(expected.tag),
      );

      if (expected.signature === null) {
        expect(frame.signature).toBeUndefined();
      } else {
        expect(frame.signature).toEqual(
          hex(expected.signature),
        );
      }

      expect(frame.serialize()).toEqual(
        hex(expected.frame),
      );
    },
  );

  it.each(testVectors.frames)(
    "$name decrypts to the expected plaintext",
    async (expected) => {
      const fields = headerFields(expected.header);
      const signed = fields.sigFlag === 1;

      const lease = { remaining: 0 };
      const publicKey = signed
        ? await ed25519.getPublicKeyAsync(
            hex(testVectors.ed25519Seed),
          )
        : new Uint8Array();

      const plaintext = await decryptFrame(
        fields.encrypted === 1
          ? storeWithKey()
          : new InMemoryKeyStore(),
        publicKey,
        lease,
        hex(expected.frame),
      );

      expect(plaintext).toEqual(
        hex(expected.plaintext),
      );

      expect(lease.remaining).toBe(
        expected.lease,
      );
    },
  );

  it("encodes pad_len at the beginning of the payload", () => {
    const expected = vector(
      "encrypted_unsigned_binary",
    );

    expect(expected.padLen).toBe(3);

    expect(expected.payload.slice(0, 8)).toBe(
      "00000003",
    );

    expect(expected.payload.slice(8, 14)).toBe(
      "000000000000",
    );
  });

  it("encodes an empty payload with pad_len zero", () => {
    const expected = vector(
      "encrypted_unsigned_empty",
    );

    expect(expected.payload).toBe("00000000");
    expect(expected.plaintext).toBe("");
  });

  it("places the tag before the signature", () => {
    const expected = vector("encrypted_signed");
    const fields = headerFields(expected.header);
    const serialized = hex(expected.frame);
    const payload = hex(expected.payload);
    const tag = hex(expected.tag);
    const signature = hex(expected.signature!);

    const payloadOffset = fields.bytes.length;
    const tagOffset = payloadOffset + payload.length;
    const signatureOffset = tagOffset + tag.length;

    expect(serialized.slice(
      payloadOffset,
      tagOffset,
    )).toEqual(payload);

    expect(serialized.slice(
      tagOffset,
      signatureOffset,
    )).toEqual(tag);

    expect(serialized.slice(
      signatureOffset,
    )).toEqual(signature);
  });
});

describe("Frame errors", () => {
  it("rejects a frame shorter than the header", () => {
    expect(() => Frame.parse(new Uint8Array(16)))
      .toThrowError(
        expect.objectContaining({
          code: "TruncatedFrame",
        }),
      );
  });

  it("rejects an unsupported version", () => {
    const encoded = hex(
      vector("encrypted_unsigned_empty").frame,
    );

    encoded[4] = 99;

    expect(() => Frame.parse(encoded))
      .toThrowError(
        expect.objectContaining({
          code: "UnsupportedVersion",
        }),
      );
  });

  it("rejects tampered encrypted frame data", async () => {
    const expected = vector(
      "encrypted_unsigned_binary",
    );

    const encoded = hex(expected.frame);
    encoded[encoded.length - 1] ^= 1;

    await expect(
      decryptFrame(
        storeWithKey(),
        new Uint8Array(),
        { remaining: 0 },
        encoded,
      ),
    ).rejects.toThrow();
  });

  it("rejects an unknown encryption key", async () => {
    const expected = vector(
      "encrypted_unsigned_empty",
    );

    await expect(
      decryptFrame(
        new InMemoryKeyStore(),
        new Uint8Array(),
        { remaining: 0 },
        hex(expected.frame),
      ),
    ).rejects.toThrowError(
      expect.objectContaining({
        code: "InvalidKeyId",
      }),
    );
  });

  it("rejects a missing encrypted-frame tag", () => {
    const expected = vector(
      "encrypted_unsigned_empty",
    );

    const encoded = hex(expected.frame);
    encoded.set(
      encoded.slice(0, encoded.length - 16),
    );

    expect(() => Frame.parse(
      encoded.slice(0, encoded.length - 16),
    )).toThrow();
  });

  it("rejects a missing signed-frame signature", () => {
    const expected = vector("cleartext_signed");
    const encoded = hex(expected.frame);

    expect(() => Frame.parse(
      encoded.slice(0, encoded.length - 64),
    )).toThrow();
  });
});
