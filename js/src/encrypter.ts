import { EncryptionType } from "./constants.js";
import { MoqSecureError } from "./errors.js";
import { encryptFrame } from "./wire.js";
import type { KeyStore } from "./keys.js";

const MAX_U64 = 0xffff_ffff_ffff_ffffn;

export interface FrameEncrypter {
  /**
   * Encrypts one complete application payload.
   *
   * The sequence number is retained for compatibility with
   * media-frame encrypters. moq-secure uses its own counter.
   */
  encrypt(
    sequenceNumber: bigint | number,
    plaintext: Uint8Array,
  ): Promise<Uint8Array>;
}

export interface MoqSecureEncrypterProps {
  keyStore: KeyStore;
  signingPrivateKey: Uint8Array;
  keyId: number;

  /**
   * Encryption type stored in the frame wire header.
   *
   * Defaults to AES-256-GCM.
   */
  encryptionType?: EncryptionType;

  /**
   * Sign every nSigned-th frame.
   *
   * 0: signing disabled
   * 1: every frame signed
   * 2: every second frame signed
   * 3: every third frame signed
   */
  nSigned: number;

  padLen: number;
  initialCtr?: bigint;
}

/**
 * Encrypts each complete application frame using moq-secure.
 */
export class MoqSecureEncrypter implements FrameEncrypter {
  readonly keyStore: KeyStore;
  readonly signingPrivateKey: Uint8Array;
  readonly keyId: number;
  readonly encryptionType: EncryptionType;
  readonly nSigned: number;
  readonly padLen: number;

  #ctr: bigint;
  #frameCount = 0;

  constructor(props: MoqSecureEncrypterProps) {
    if (
      !Number.isInteger(props.keyId) ||
      props.keyId < 0 ||
      props.keyId > 255
    ) {
      throw new RangeError("keyId must be an unsigned byte");
    }

    if (
      !Number.isInteger(props.nSigned) ||
      props.nSigned < 0 ||
      props.nSigned > 255
    ) {
      throw new RangeError("nSigned must be an unsigned byte");
    }

    if (
      !Number.isInteger(props.padLen) ||
      props.padLen < 0 ||
      props.padLen > 0xffff_ffff
    ) {
      throw new RangeError("padLen must be a uint32");
    }

    const initialCtr = props.initialCtr ?? 0n;

    if (initialCtr < 0n || initialCtr > MAX_U64) {
      throw new RangeError(
        "initialCtr must fit in an unsigned 64-bit integer",
      );
    }

    this.keyStore = props.keyStore;
    this.signingPrivateKey = props.signingPrivateKey.slice();
    this.keyId = props.keyId;
    this.encryptionType =
      props.encryptionType ?? EncryptionType.AES_256_GCM;
    this.nSigned = props.nSigned;
    this.padLen = props.padLen;
    this.#ctr = initialCtr;
  }

  nextCounter(): bigint {
    return this.#ctr;
  }

  #takeCounter(): bigint {
    if (this.#ctr === MAX_U64) {
      throw MoqSecureError.counterExhausted();
    }

    const counter = this.#ctr;
    this.#ctr += 1n;
    return counter;
  }

  #shouldSign(): boolean {
    /*
     * nSigned == 0 means signing is disabled. Avoid modulo zero.
     */
    if (this.nSigned === 0) {
      return false;
    }

    /*
     * #frameCount is zero-based:
     *
     * nSigned = 1: frames 1, 2, 3, ...
     * nSigned = 2: frames 1, 3, 5, ...
     * nSigned = 3: frames 1, 4, 7, ...
     */
    return this.#frameCount % this.nSigned === 0;
  }

  async encrypt(
    _sequenceNumber: bigint | number,
    plaintext: Uint8Array,
  ): Promise<Uint8Array> {
    const ctr = this.#takeCounter();
    const shouldSign = this.#shouldSign();

    /*
     * Allocate the frame position synchronously, before awaiting
     * encryption or signing. This keeps the schedule correct even if
     * encrypt() is called concurrently.
     */
    this.#frameCount++;

    const frame = await encryptFrame(
      this.keyStore,
      this.signingPrivateKey,
      this.keyId,
      ctr,
      this.nSigned,
      shouldSign,
      this.encryptionType,
      this.padLen,
      plaintext,
    );

    return frame.serialize();
  }
}
