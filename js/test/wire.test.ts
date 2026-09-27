import { describe, expect, it } from "vitest";
import * as ed25519 from "@noble/ed25519";

import vectors from "../../test-vectors/frames.json";

import {
  Frame,
  WireHeader,
  decryptFrame,
  encryptFrame,
  type EncryptionType,
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
  initialLease: number;
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
    encryptionType: header[16],
  };
}

function storeWithKey(): InMemoryKeyStore {
  const store = new InMemoryKeyStore();

  store.setKey(
    7,
    hex(testVectors.aeadKey),
  );

  return store;
}

async function publicKey(): Promise<Uint8Array> {
  return ed25519.getPublicKeyAsync(
    hex(testVectors.ed25519Seed),
  );
}

async function makeFrame(
  ctr: bigint,
  nSigned: number,
  signed: boolean,
  encryptionType: EncryptionType = 1,
): Promise<Frame> {
  return encryptFrame(
    storeWithKey(),
    hex(testVectors.ed25519Seed),
    7,
    ctr,
    nSigned,
    signed,
    encryptionType,
    0,
    new Uint8Array([0]),
  );
}

async function decryptGeneratedFrame(
  frame: Frame,
  lease: { remaining: number },
) {
  return decryptFrame(
    storeWithKey(),
    frame.header.sigFlag === 1
      ? await publicKey()
      : new Uint8Array(),
    lease,
    frame.serialize(),
  );
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
      0, 0, 0, 0,
      ...new Uint8Array(16),
    ]);

    const parsed = Frame.parse(encoded).header;

    expect(parsed.magic).toEqual(MAGIC);
    expect(parsed.version).toBe(VERSION);
    expect(parsed.keyId).toBe(7);
    expect(parsed.ctr).toBe(42n);
    expect(parsed.nSigned).toBe(3);
    expect(parsed.sigFlag).toBe(0);
    expect(parsed.encryptionType).toBe(1);
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
        fields.encryptionType,
        expected.padLen,
        hex(expected.plaintext),
      );

      expect(frame.header.encode()).toEqual(fields.bytes);
      expect(frame.payload).toEqual(hex(expected.payload));
      expect(frame.tag).toEqual(hex(expected.tag));

      if (expected.signature === null) {
        expect(frame.signature).toBeUndefined();
      } else {
        expect(frame.signature).toEqual(
          hex(expected.signature),
        );
      }

      expect(frame.serialize()).toEqual(hex(expected.frame));
    },
  );

  it.each(testVectors.frames)(
    "$name decrypts to the expected plaintext and lease",
    async (expected) => {
      const fields = headerFields(expected.header);
      const signed = fields.sigFlag === 1;

      const lease = {
        remaining: expected.initialLease,
      };

      const plaintext = await decryptFrame(
        fields.encryptionType !== 0
          ? storeWithKey()
          : new InMemoryKeyStore(),
        signed
          ? await publicKey()
          : new Uint8Array(),
        lease,
        hex(expected.frame),
      );

      expect(plaintext).toEqual(hex(expected.plaintext));
      expect(lease.remaining).toBe(expected.lease);
      expect(lease.remaining).toBeGreaterThanOrEqual(0);
    },
  );
});

describe("AES-256-GCM frames", () => {
  it("round-trips an unsigned encrypted frame", async () => {
    const plaintext = new Uint8Array([
      0x00, 0x01, 0x7f, 0x80, 0xff,
    ]);

    const frame = await encryptFrame(
      storeWithKey(),
      hex(testVectors.ed25519Seed),
      7,
      100n,
      0,
      false,
      2,
      3,
      plaintext,
    );

    expect(frame.header.encryptionType).toBe(2);
    expect(frame.tag).toHaveLength(16);

    await expect(
      decryptFrame(
        storeWithKey(),
        new Uint8Array(),
        { remaining: 0 },
        frame.serialize(),
      ),
    ).resolves.toEqual(plaintext);
  });

  it("round-trips a signed encrypted frame", async () => {
    const plaintext = new Uint8Array([1, 2, 3]);

    const frame = await encryptFrame(
      storeWithKey(),
      hex(testVectors.ed25519Seed),
      7,
      101n,
      1,
      true,
      2,
      0,
      plaintext,
    );

    await expect(
      decryptFrame(
        storeWithKey(),
        await publicKey(),
        { remaining: 0 },
        frame.serialize(),
      ),
    ).resolves.toEqual(plaintext);
  });

  it("rejects an AES-GCM frame when the algorithm field is modified", async () => {
    const frame = await encryptFrame(
      storeWithKey(),
      hex(testVectors.ed25519Seed),
      7,
      102n,
      0,
      false,
      2,
      0,
      new Uint8Array([1, 2, 3]),
    );

    const serialized = frame.serialize();

    // Change AES-256-GCM (2) to ChaCha20-Poly1305 (1).
    serialized[16] = 1;

    await expect(
      decryptFrame(
        storeWithKey(),
        new Uint8Array(),
        { remaining: 0 },
        serialized,
      ),
    ).rejects.toThrow();
  });
});

describe("signed-frame lease", () => {
  it("sets lease_remaining to nSigned - 1 after a signed frame", async () => {
    const frame = await makeFrame(1n, 4, true);
    const lease = { remaining: 0 };

    await decryptGeneratedFrame(frame, lease);

    expect(lease.remaining).toBe(3);
  });

  it("sets lease_remaining to zero when nSigned is one", async () => {
    const frame = await makeFrame(1n, 1, true);
    const lease = { remaining: 99 };

    await decryptGeneratedFrame(frame, lease);

    expect(lease.remaining).toBe(0);
  });

  it("accepts no unsigned frames when nSigned is one", async () => {
    const signed = await makeFrame(1n, 1, true);
    const unsigned = await makeFrame(2n, 1, false);
    const lease = { remaining: 0 };

    await decryptGeneratedFrame(signed, lease);

    await expect(
      decryptGeneratedFrame(unsigned, lease),
    ).rejects.toThrow();

    expect(lease.remaining).toBe(0);
  });

  it("accepts exactly nSigned - 1 unsigned frames", async () => {
    const signed = await makeFrame(1n, 3, true);
    const unsigned1 = await makeFrame(2n, 3, false);
    const unsigned2 = await makeFrame(3n, 3, false);
    const unsigned3 = await makeFrame(4n, 3, false);
    const lease = { remaining: 0 };

    await decryptGeneratedFrame(signed, lease);
    expect(lease.remaining).toBe(2);

    await decryptGeneratedFrame(unsigned1, lease);
    expect(lease.remaining).toBe(1);

    await decryptGeneratedFrame(unsigned2, lease);
    expect(lease.remaining).toBe(0);

    await expect(
      decryptGeneratedFrame(unsigned3, lease),
    ).rejects.toThrow();

    expect(lease.remaining).toBe(0);
  });

  it("does not apply the lease when signing is disabled", async () => {
    const unsigned1 = await makeFrame(1n, 0, false);
    const unsigned2 = await makeFrame(2n, 0, false);
    const unsigned3 = await makeFrame(3n, 0, false);
    const lease = { remaining: 7 };

    await decryptGeneratedFrame(unsigned1, lease);
    await decryptGeneratedFrame(unsigned2, lease);
    await decryptGeneratedFrame(unsigned3, lease);

    expect(lease.remaining).toBe(7);
  });

  it("never makes lease_remaining negative", async () => {
    const signed = await makeFrame(1n, 2, true);
    const unsigned1 = await makeFrame(2n, 2, false);
    const unsigned2 = await makeFrame(3n, 2, false);
    const lease = { remaining: 0 };

    await decryptGeneratedFrame(signed, lease);
    expect(lease.remaining).toBe(1);

    await decryptGeneratedFrame(unsigned1, lease);
    expect(lease.remaining).toBe(0);

    await expect(
      decryptGeneratedFrame(unsigned2, lease),
    ).rejects.toThrow();

    expect(lease.remaining).toBe(0);
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
    const encoded = hex(
      vector("encrypted_unsigned_binary").frame,
    );

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
    const expected = vector("encrypted_unsigned_empty");

    await expect(
      decryptFrame(
        new InMemoryKeyStore(),
        new Uint8Array(),
        { remaining: expected.initialLease },
        hex(expected.frame),
      ),
    ).rejects.toThrowError(
      expect.objectContaining({
        code: "InvalidKeyId",
      }),
    );
  });

  it("rejects a missing encrypted-frame tag", () => {
    const encoded = hex(
      vector("encrypted_unsigned_empty").frame,
    );

    expect(() => Frame.parse(
      encoded.slice(0, encoded.length - 16),
    )).toThrow();
  });

  it("rejects a missing signed-frame signature", () => {
    const encoded = hex(
      vector("cleartext_signed").frame,
    );

    expect(() => Frame.parse(
      encoded.slice(0, encoded.length - 64),
    )).toThrow();
  });
});
