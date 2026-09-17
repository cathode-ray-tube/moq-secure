import { MoqSecureError } from "./errors.js";
import type { KeyStore } from "./keys.js";
import { decryptFrame, encryptFrame } from "./wire.js";

const MAX_U64 = 0xffff_ffff_ffff_ffffn;

export interface MoqSecureEncrypterConfig {
  keyStore: KeyStore;
  signingPrivateKey: Uint8Array;
  keyId: number;
  nSigned: number;
  maybeSign: boolean;
  padLen: number;
  initialCtr?: bigint;
}

/**
 * Encrypts each frame using moq-secure.
 *
 * The sequence number passed to encrypt() is intentionally ignored.
 * moq-secure uses its own independent 64-bit encryption counter.
 */
export class MoqSecureEncrypter {
  readonly keyStore: KeyStore;
  readonly signingPrivateKey: Uint8Array;
  readonly keyId: number;
  readonly nSigned: number;
  readonly maybeSign: boolean;
  readonly padLen: number;

  private ctr: bigint;

  constructor(config: MoqSecureEncrypterConfig) {
    if (
      !Number.isInteger(config.keyId) ||
      config.keyId < 0 ||
      config.keyId > 255
    ) {
      throw new RangeError("keyId must be an unsigned byte");
    }

    if (
      !Number.isInteger(config.nSigned) ||
      config.nSigned < 0 ||
      config.nSigned > 255
    ) {
      throw new RangeError("nSigned must be an unsigned byte");
    }

    if (
      !Number.isInteger(config.padLen) ||
      config.padLen < 0 ||
      config.padLen > 0xffff_ffff
    ) {
      throw new RangeError("padLen must be a uint32");
    }

    const initialCtr = config.initialCtr ?? 0n;

    if (
      initialCtr < 0n ||
      initialCtr > MAX_U64
    ) {
      throw new RangeError("initialCtr must fit in an unsigned 64-bit integer");
    }

    this.keyStore = config.keyStore;
    this.signingPrivateKey = config.signingPrivateKey.slice();
    this.keyId = config.keyId;
    this.nSigned = config.nSigned;
    this.maybeSign = config.maybeSign;
    this.padLen = config.padLen;
    this.ctr = initialCtr;
  }

  nextCounter(): bigint {
    return this.ctr;
  }

  private takeCounter(): bigint {
    if (this.ctr === MAX_U64) {
      throw MoqSecureError.counterExhausted();
    }

    const result = this.ctr;
    this.ctr += 1n;
    return result;
  }

  async encrypt(
    _sequenceNumber: bigint | number,
    plaintext: Uint8Array,
  ): Promise<Uint8Array> {
    const ctr = this.takeCounter();

    const frame = await encryptFrame(
      this.keyStore,
      this.signingPrivateKey,
      this.keyId,
      ctr,
      this.nSigned,
      this.maybeSign,
      1,
      this.padLen,
      plaintext,
    );

    return frame.serialize();
  }
}

export interface MoqSecureDecryptionConfig {
  keyStore: KeyStore;
  broadcasterPublicKey: Uint8Array;
}

/**
 * Decrypts and verifies each frame using moq-secure.
 */
export class MoqSecureDecrypter {
  readonly keyStore: KeyStore;
  readonly broadcasterPublicKey: Uint8Array;

  private leaseRemainingValue: number;

  constructor(config: MoqSecureDecryptionConfig) {
    this.keyStore = config.keyStore;
    this.broadcasterPublicKey = config.broadcasterPublicKey.slice();
    this.leaseRemainingValue = 0;
  }

  static withLease(
    keyStore: KeyStore,
    broadcasterPublicKey: Uint8Array,
    leaseRemaining: number,
  ): MoqSecureDecrypter {
    const result = new MoqSecureDecrypter({
      keyStore,
      broadcasterPublicKey,
    });

    result.setLease(leaseRemaining);
    return result;
  }

  leaseRemaining(): number {
    return this.leaseRemainingValue;
  }

  resetLease(): void {
    this.leaseRemainingValue = 0;
  }

  private setLease(value: number): void {
    if (
      !Number.isInteger(value) ||
      value < 0 ||
      value > 255
    ) {
      throw new RangeError("leaseRemaining must be an unsigned byte");
    }

    this.leaseRemainingValue = value;
  }

  async decrypt(
    _sequenceNumber: bigint | number,
    ciphertext: Uint8Array,
  ): Promise<Uint8Array> {
    const lease = {
      remaining: this.leaseRemainingValue,
    };

    const plaintext = await decryptFrame(
      this.keyStore,
      this.broadcasterPublicKey,
      lease,
      ciphertext,
    );

    this.leaseRemainingValue = lease.remaining;
    return plaintext;
  }
}
