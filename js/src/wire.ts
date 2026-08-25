import { sign, verify } from "@noble/ed25519";

import {
  AEAD_TAG_LEN,
  FIXED_HEADER_LEN,
  MAGIC,
  SIG_SLOT_LEN,
  VERSION,
} from "./constants.js";
import { aeadDecrypt, aeadEncrypt, sha256Digest } from "./crypto.js";
import { MoqSecureError } from "./errors.js";
import { KeyStore } from "./keys.js";
import {
  prependZeroPadding,
  removeZeroPadding,
} from "./padding.js";

function equalBytes(a: Uint8Array, b: Uint8Array): boolean {
  return a.length === b.length && a.every((v, i) => v === b[i]);
}

function concat(...parts: Uint8Array[]): Uint8Array {
  const result = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
  }
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
    public readonly padLen: number,
  ) {}

  encode(): Uint8Array {
    const result = new Uint8Array(FIXED_HEADER_LEN);
    let i = 0;

    result.set(this.magic, i); i += 4;
    result[i++] = this.version;
    result[i++] = this.keyId;
    new DataView(result.buffer).setBigUint64(i, this.ctr, false);
    i += 8;
    result[i++] = this.nSigned;
    result[i++] = this.sigFlag;
    result[i++] = this.encrypted;
    new DataView(result.buffer).setUint32(i, this.padLen, false);

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
    this.tag = (init.tag ?? new Uint8Array(AEAD_TAG_LEN)).slice();
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
      view.getUint32(17, false),
    );

    header.validate();

    const trailerLength = header.sigFlag === 1 ? SIG_SLOT_LEN : 0;
    if (input.length < FIXED_HEADER_LEN + trailerLength) {
      throw MoqSecureError.truncated();
    }

    const bodyEnd = input.length - trailerLength;
    const body = input.slice(FIXED_HEADER_LEN, bodyEnd);
    let signature: Uint8Array | undefined;

    if (trailerLength) {
      signature = input.slice(bodyEnd);
      if (signature.every((b) => b === 0)) {
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

      return new Frame({
        header,
        payload: body.slice(0, -AEAD_TAG_LEN),
        tag: body.slice(-AEAD_TAG_LEN),
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

    return concat(this.header.encode(), body, signature);
  }

  aadBytes(): Uint8Array {
    return this.header.aad();
  }

  digestForSignature(): Uint8Array {
    return sha256Digest(
      this.header.encrypted === 1
        ? concat(this.header.encode(), this.payload, this.tag)
        : concat(this.header.encode(), this.payload),
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
      if (!this.signature) throw MoqSecureError.invalidSignature();

      const valid = await verify(
        this.signature,
        this.digestForSignature(),
        broadcasterPublicKey,
      );

      if (!valid) throw MoqSecureError.invalidSignature();
      lease.remaining = this.header.nSigned;
    } else {
      if (lease.remaining === 0) throw MoqSecureError.invalidSignature();
      lease.remaining--;
    }

    let padded: Uint8Array;

    if (this.header.encrypted === 1) {
      const key = keyStore.aeadKey(this.header.keyId);
      if (!key) {
        throw new MoqSecureError(
          "InvalidKeyId",
          `unknown or not-loaded key_id: ${this.header.keyId}`,
          this.header.keyId,
        );
      }

      padded = aeadDecrypt(
        key,
        this.header.keyId,
        this.header.ctr,
        this.aadBytes(),
        this.payload,
        this.tag,
      );
    } else {
      padded = this.payload;
    }

    try {
      return removeZeroPadding(padded, this.header.padLen);
    } catch {
      throw MoqSecureError.authFailed();
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

  const key = keyStore.aeadKey(keyId);
  if (!key) {
    throw new MoqSecureError(
      "InvalidKeyId",
      `unknown or not-loaded key_id: ${keyId}`,
      keyId,
    );
  }

  const sigFlag = nSigned === 0 ? 0 : maybeSign ? 1 : 0;

  const header = new WireHeader(
    MAGIC,
    VERSION,
    keyId,
    ctr,
    nSigned,
    sigFlag,
    encrypted,
    padLen,
  );

  header.validate();

  const padded = prependZeroPadding(plaintext, padLen);
  let payload: Uint8Array;
  let tag = new Uint8Array(AEAD_TAG_LEN);

  if (encrypted === 1) {
    const result = aeadEncrypt(
      key,
      keyId,
      ctr,
      header.aad(),
      padded,
    );
    payload = result.ciphertext;
    tag = result.tag;
  } else {
    payload = padded;
  }

  const unsigned = new Frame({ header, payload, tag });

  if (sigFlag === 0) return unsigned;

  return new Frame({
    header,
    payload,
    tag,
    signature: await sign(
      unsigned.digestForSignature(),
      broadcasterPrivateKey,
    ),
  });
}

export async function decryptFrame(
  keyStore: KeyStore,
  broadcasterPublicKey: Uint8Array,
  lease: { remaining: number },
  frameBytes: Uint8Array,
): Promise<Uint8Array> {
  return Frame.parse(frameBytes).decodePlaintext(
    keyStore,
    broadcasterPublicKey,
    lease,
  );
}
