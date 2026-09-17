import { MoqSecureError } from "../secure/errors.js";
import { encryptFrame } from "../secure/wire.js";
import type { KeyStore } from "../secure/keys.js";

const MAX_U64 = 0xffff_ffff_ffff_ffffn;

export interface FrameEncrypter {
	/**
	 * Encrypt a complete legacy-container payload.
	 *
	 * The plaintext is:
	 *
	 *   timestamp varint || codec payload
	 *
	 * sequenceNumber is accepted for API compatibility with MoQ frame
	 * encrypters. Moq Secure uses its own independent counter.
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
	nSigned: number;
	maybeSign: boolean;
	padLen: number;
	initialCtr?: bigint;
}

/**
 * Encrypts each complete legacy-container frame using moq-secure.
 */
export class MoqSecureEncrypter implements FrameEncrypter {
	readonly keyStore: KeyStore;
	readonly signingPrivateKey: Uint8Array;
	readonly keyId: number;
	readonly nSigned: number;
	readonly maybeSign: boolean;
	readonly padLen: number;

	#ctr: bigint;

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
		this.nSigned = props.nSigned;
		this.maybeSign = props.maybeSign;
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

	async encrypt(
		_sequenceNumber: bigint | number,
		plaintext: Uint8Array,
	): Promise<Uint8Array> {
		const ctr = this.#takeCounter();

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
