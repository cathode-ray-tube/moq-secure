import { decryptFrame } from "../secure/wire.js";
import type { KeyStore } from "../secure/keys.js";

export interface FrameDecrypter {
	/**
	 * Decrypt and authenticate a complete moq-secure frame.
	 *
	 * The returned plaintext is:
	 *
	 *   timestamp varint || codec payload
	 */
	decrypt(
		sequenceNumber: bigint | number,
		ciphertext: Uint8Array,
	): Promise<Uint8Array>;
}

export interface MoqSecureDecrypterProps {
	keyStore: KeyStore;
	broadcasterPublicKey: Uint8Array;
}

/**
 * Decrypts and verifies each moq-secure frame.
 */
export class MoqSecureDecrypter implements FrameDecrypter {
	readonly keyStore: KeyStore;
	readonly broadcasterPublicKey: Uint8Array;

	#leaseRemaining = 0;

	constructor(props: MoqSecureDecrypterProps) {
		this.keyStore = props.keyStore;
		this.broadcasterPublicKey = props.broadcasterPublicKey.slice();
	}

	static withLease(
		keyStore: KeyStore,
		broadcasterPublicKey: Uint8Array,
		leaseRemaining: number,
	): MoqSecureDecrypter {
		const decrypter = new MoqSecureDecrypter({
			keyStore,
			broadcasterPublicKey,
		});

		decrypter.#setLease(leaseRemaining);
		return decrypter;
	}

	leaseRemaining(): number {
		return this.#leaseRemaining;
	}

	resetLease(): void {
		this.#leaseRemaining = 0;
	}

	#setLease(value: number): void {
		if (
			!Number.isInteger(value) ||
			value < 0 ||
			value > 255
		) {
			throw new RangeError(
				"leaseRemaining must be an unsigned byte",
			);
		}

		this.#leaseRemaining = value;
	}

	async decrypt(
		_sequenceNumber: bigint | number,
		ciphertext: Uint8Array,
	): Promise<Uint8Array> {
		const lease = {
			remaining: this.#leaseRemaining,
		};

		const plaintext = await decryptFrame(
			this.keyStore,
			this.broadcasterPublicKey,
			lease,
			ciphertext,
		);

		this.#leaseRemaining = lease.remaining;
		return plaintext;
	}
}
