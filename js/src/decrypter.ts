import { decryptFrame } from "./wire.js";
import type { KeyStore } from "./keys.js";

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
 *
 * Decryption is serialized so that lease updates cannot race when multiple
 * frames are submitted concurrently.
 */
export class MoqSecureDecrypter implements FrameDecrypter {
	readonly keyStore: KeyStore;
	readonly broadcasterPublicKey: Uint8Array;

	#leaseRemaining = 0;

	/*
	 * Each decrypt() call waits for the previous call to finish before
	 * reading or updating #leaseRemaining.
	 */
	#decryptTail: Promise<void> = Promise.resolve();

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
		let releaseTurn!: () => void;

		const currentTurn = new Promise<void>((resolve) => {
			releaseTurn = resolve;
		});

		/*
		 * Keep the queue alive even if the previous decrypt failed.
		 * A failed frame must not prevent later frames from being processed.
		 */
		const previousTurn = this.#decryptTail;

		this.#decryptTail = previousTurn
			.catch(() => undefined)
			.then(() => currentTurn);

		await previousTurn.catch(() => undefined);

		try {
			/*
			 * Use a local lease object. decryptFrame() updates this object
			 * only after the frame has been fully authenticated, decrypted,
			 * and padding-validated.
			 */
			const lease = {
				remaining: this.#leaseRemaining,
			};

			const plaintext = await decryptFrame(
				this.keyStore,
				this.broadcasterPublicKey,
				lease,
				ciphertext,
			);

			/*
			 * Commit the lease only after decryptFrame() succeeds.
			 *
			 * If signature verification, AEAD authentication, or padding
			 * validation fails, decryptFrame() throws and this assignment is
			 * skipped. The previous lease is therefore preserved.
			 */
			this.#leaseRemaining = lease.remaining;

			return plaintext;
		} finally {
			/*
			 * Allow the next queued decrypt() call to run regardless of
			 * whether this call succeeded or failed.
			 */
			releaseTurn();
		}
	}
}
