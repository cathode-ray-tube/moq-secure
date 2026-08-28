import { createHash } from "node:crypto";
import * as ed25519 from "@noble/ed25519";

import {
  AEAD_TAG_LEN,
  FIXED_HEADER_LEN,
  MAGIC,
  SIG_SLOT_LEN,
  VERSION,
} from "./constants.js";
import { aeadDecrypt, aeadEncrypt, sha256Digest } from "./crypto.js";
import { MoqSecureError } from "./errors.js";
import type { KeyStore } from "./keys.js";
import {
  prependZeroPadding,
  removePadding,
} from "./padding.js";

ed25519.hashes.sha512 = (...messages: Uint8Array[]) => {
  const hash = createHash("sha512");

  for (const message of messages) {
    hash.update(message);
  }

  return new Uint8Array(hash.digest());
};

function equalBytes(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length &&
    a.every((value, index) => value === b[index]);
}

function concat(...parts: Uint8Array[]): Uint8Array {
  const result = new Uint8Array(
    parts.reduce((length, part) => length + part.length, 0),
  );

  let offset = 0;

  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
  }

  return result;
}

function encodeUint32BE(value: number): Uint8Array {
  if (
    !Number.isSafeInteger(value) ||
    value < 0 ||
    value > 0xffffffff
  ) {
    throw new RangeError("padLen must be a uint32");
  }

  const result = new Uint8Array(4);
  new DataView(result.buffer).setUint32(0, value, false);
  return result;
}

export class WireHeader {
  constructor(
    public readonly magic: Uint8Array,
    public readonly version: number,
    public readonly keyId: number,
    public readonly ctr: bigint,
    public readonly nSigned: number,
    public readonly sigFlag: number,
    public readonly encrypted: number,
  ) {}

  encode(): Uint8Array {
    const result = new Uint8Array(FIXED_HEADER_LEN);
    let offset = 0;

    result.set(this.magic, offset);
    offset += 4;

    result[offset++] = this.version;
    result[offset++] = this.keyId;

    new DataView(result.buffer).setBigUint64(
      offset,
      this.ctr,
      false,
    );
    offset += 8;

    result[offset++] = this.nSigned;
    result[offset++] = this.sigFlag;
    result[offset++] = this.encrypted;

    return result;
  }

  aad(): Uint8Array {
    return this.encode();
  }

  validate(): void {
    if (!equalBytes(this.magic, MAGIC)) {
      throw MoqSecureError.invalidMagic();
    }

    if (this.version !== VERSION) {
      throw MoqSecureError.unsupportedVersion(this.version);
    }

    if (this.sigFlag !== 0 && this.sigFlag !== 1) {
      throw new MoqSecureError(
        "InvalidSigFlag",
        `sigFlag must be 0 or 1, got ${this.sigFlag}`,
      );
    }

    if (this.encrypted !== 0 && this.encrypted !== 1) {
      throw new MoqSecureError(
        "InvalidEncryptedFlag",
        `encrypted flag must be 0 or 1, got ${this.encrypted}`,
      );
    }

    if (this.nSigned === 0 && this.sigFlag !== 0) {
      throw new MoqSecureError(
        "SigningMismatch",
        "signing is disabled but sigFlag indicates a signature",
      );
    }
  }
}

export interface FrameInit {
  header: WireHeader;
  payload: Uint8Array;
  tag?: Uint8Array;
  signature?: Uint8Array;
}

export class Frame {
  readonly header: WireHeader;
  readonly payload: Uint8Array;
  readonly tag: Uint8Array;
  readonly signature?: Uint8Array;

  constructor(init: FrameInit) {
    this.header = init.header;
    this.payload = init.payload.slice();
    this.tag = (
      init.tag ?? new Uint8Array(AEAD_TAG_LEN)
    ).slice();
    this.signature = init.signature?.slice();
  }

  static parse(input: Uint8Array): Frame {
    if (input.length < FIXED_HEADER_LEN) {
      throw MoqSecureError.truncated();
    }

    const view = new DataView(
      input.buffer,
      input.byteOffset,
      input.byteLength,
    );

    const header = new WireHeader(
      input.slice(0, 4),
      input[4],
      input[5],
      view.getBigUint64(6, false),
      input[14],
      input[15],
      input[16],
    );

    header.validate();

    const signatureLength = header.sigFlag === 1
      ? SIG_SLOT_LEN
      : 0;

    if (input.length < FIXED_HEADER_LEN + signatureLength) {
      throw MoqSecureError.truncated();
    }

    const bodyEnd = input.length - signatureLength;
    const body = input.slice(FIXED_HEADER_LEN, bodyEnd);

    let signature: Uint8Array | undefined;

    if (signatureLength > 0) {
      signature = input.slice(bodyEnd);

      if (signature.length !== SIG_SLOT_LEN) {
        throw MoqSecureError.truncated();
      }

      if (signature.every((byte) => byte === 0)) {
        throw MoqSecureError.invalidSignature();
      }
    }

    if (header.encrypted === 1) {
      if (body.length < AEAD_TAG_LEN) {
        throw new MoqSecureError(
          "CiphertextTooShort",
          "ciphertext too short for AEAD tag",
        );
      }

      const ciphertextEnd = body.length - AEAD_TAG_LEN;

      return new Frame({
        header,
        payload: body.slice(0, ciphertextEnd),
        tag: body.slice(ciphertextEnd),
        signature,
      });
    }

    return new Frame({
      header,
      payload: body,
      signature,
    });
  }

  serialize(): Uint8Array {
    const body = this.header.encrypted === 1
      ? concat(this.payload, this.tag)
      : this.payload;

    const signature = this.header.sigFlag === 1
      ? this.signature ?? new Uint8Array(SIG_SLOT_LEN)
      : new Uint8Array();

    return concat(
      this.header.encode(),
      body,
      signature,
    );
  }

  aadBytes(): Uint8Array {
    return this.header.aad();
  }

  digestForSignature(): Uint8Array {
    const body = this.header.encrypted === 1
      ? concat(this.payload, this.tag)
      : this.payload;

    return sha256Digest(
      concat(this.header.encode(), body),
    );
  }

  async decodePlaintext(
    keyStore: KeyStore,
    broadcasterPublicKey: Uint8Array,
    lease: { remaining: number },
  ): Promise<Uint8Array> {
    if (this.header.nSigned === 0) {
      if (this.header.sigFlag !== 0 || this.signature) {
        throw new MoqSecureError(
          "SigningMismatch",
          "signing is disabled but a signature is present",
        );
      }
    } else if (this.header.sigFlag === 1) {
      if (!this.signature) {
        throw MoqSecureError.invalidSignature();
      }

      const valid = await ed25519.verify(
        this.signature,
        this.digestForSignature(),
        broadcasterPublicKey,
      );

      if (!valid) {
        throw MoqSecureError.invalidSignature();
      }

      lease.remaining = this.header.nSigned;
    } else {
      if (lease.remaining === 0) {
        throw MoqSecureError.invalidSignature();
      }

      lease.remaining--;
    }

    let paddedPlaintext: Uint8Array;

    if (this.header.encrypted === 1) {
      const key = keyStore.aeadKey(this.header.keyId);

      if (!key) {
        throw new MoqSecureError(
          "InvalidKeyId",
          `unknown or not-loaded key_id: ${this.header.keyId}`,
          this.header.keyId,
        );
      }

      // The encrypted plaintext is:
      //
      //   uint32be(padLen) || zero padding || plaintext
      //
      // The pad_len field is part of the encrypted plaintext.
      paddedPlaintext = aeadDecrypt(
        key,
        this.header.keyId,
        this.header.ctr,
        this.aadBytes(),
        this.payload,
        this.tag,
      );
    } else {
      // Cleartext payload has the same layout:
      //
      //   uint32be(padLen) || zero padding || plaintext
      paddedPlaintext = this.payload;
    }

    try {
      return removePadding(paddedPlaintext);
    } catch {
      throw new MoqSecureError(
        "InvalidPadLength",
        "invalid padding length",
      );
    }
  }
}

export async function encryptFrame(
  keyStore: KeyStore,
  broadcasterPrivateKey: Uint8Array,
  keyId: number,
  ctr: bigint,
  nSigned: number,
  maybeSign: boolean,
  encrypted: number,
  padLen: number,
  plaintext: Uint8Array,
): Promise<Frame> {
  if (encrypted !== 0 && encrypted !== 1) {
    throw new MoqSecureError(
      "InvalidEncryptedFlag",
      `encrypted flag must be 0 or 1, got ${encrypted}`,
    );
  }

  const sigFlag = nSigned !== 0 && maybeSign ? 1 : 0;

  const header = new WireHeader(
    MAGIC,
    VERSION,
    keyId,
    ctr,
    nSigned,
    sigFlag,
    encrypted,
  );

  header.validate();

  // The plaintext representation is:
  //
  //   uint32be(padLen) || zero padding || plaintext
  //
  // For encrypted frames, the complete value is encrypted.
  const paddedPlaintext = concat(
    encodeUint32BE(padLen),
    prependZeroPadding(plaintext, padLen),
  );

  let payload: Uint8Array;
  let tag = new Uint8Array(AEAD_TAG_LEN);

  if (encrypted === 1) {
    const key = keyStore.aeadKey(keyId);

    if (!key) {
      throw new MoqSecureError(
        "InvalidKeyId",
        `unknown or not-loaded key_id: ${keyId}`,
        keyId,
      );
    }

    const encryptedResult = aeadEncrypt(
      key,
      keyId,
      ctr,
      header.aad(),
      paddedPlaintext,
    );

    // The tag is stored separately by Frame.
    payload = encryptedResult.ciphertext;
    tag = encryptedResult.tag;
  } else {
    // Cleartext payload contains the same pad_len-prefixed layout.
    payload = paddedPlaintext;
  }

  const unsignedFrame = new Frame({
    header,
    payload,
    tag,
  });

  if (sigFlag === 0) {
    return unsignedFrame;
  }

  const signature = await ed25519.sign(
    unsignedFrame.digestForSignature(),
    broadcasterPrivateKey,
  );

  return new Frame({
    header,
    payload,
    tag,
    signature,
  });
}

export async function decryptFrame(
  keyStore: KeyStore,
  broadcasterPublicKey: Uint8Array,
  lease: { remaining: number },
  frameBytes: Uint8Array,
): Promise<Uint8Array> {
  const frame = Frame.parse(frameBytes);

  return frame.decodePlaintext(
    keyStore,
    broadcasterPublicKey,
    lease,
  );
}
