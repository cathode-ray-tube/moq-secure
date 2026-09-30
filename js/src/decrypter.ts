import { decryptFrame, Frame } from "./wire.js";
import type { AeadAlgorithm } from "./crypto.js";
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
	algorithm: AeadAlgorithm;
}

export type PlaybackMode = "live" | "rewind";

interface CounterState {
	/**
	 * Highest counter accepted while in live mode.
	 */
	liveMaxCtr?: bigint;

	/**
	 * Highest counter accepted during the current rewind invocation.
	 */
	playbackCtr?: bigint;
}

/**
 * Decrypts and verifies each moq-secure frame.
 *
 * Decryption is serialized so that lease updates and replay state cannot
 * race when multiple frames are submitted concurrently.
 */
export class MoqSecureDecrypter implements FrameDecrypter {
	readonly keyStore: KeyStore;
	readonly broadcasterPublicKey: Uint8Array;
	readonly algorithm: AeadAlgorithm;

	#leaseRemaining = 0;
	#mode: PlaybackMode = "live";

	/*
	 * Replay state is tracked independently for each encryption key ID.
	 */
	#counters = new Map<number, CounterState>();

	/*
	 * Each decrypt() call waits for the previous call to finish before
	 * reading or updating lease and replay state.
	 */
	#decryptTail: Promise<void> = Promise.resolve();

	constructor(props: MoqSecureDecrypterProps) {
		this.keyStore = props.keyStore;
		this.broadcasterPublicKey =
			props.broadcasterPublicKey.slice();
		this.algorithm = props.algorithm;
	}

	static withLease(
		keyStore: KeyStore,
		broadcasterPublicKey: Uint8Array,
		leaseRemaining: number,
		algorithm: AeadAlgorithm = "CHACHA20-POLY1305",
	): MoqSecureDecrypter {
		const decrypter = new MoqSecureDecrypter({
			keyStore,
			broadcasterPublicKey,
			algorithm,
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

	playbackMode(): PlaybackMode {
		return this.#mode;
	}

	/**
	 * Signals that the player has entered live playback.
	 *
	 * Live counter maxima are retained.
	 */
	beginLive(): void {
		this.#mode = "live";
	}

	/**
	 * Signals the beginning of a new rewind invocation.
	 *
	 * The rewind playback cursor is reset for every encryption key.
	 */
	beginRewind(): void {
		this.#mode = "rewind";

		for (const state of this.#counters.values()) {
			delete state.playbackCtr;
		}
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

	#checkCounter(keyId: number, ctr: bigint): void {
		const state = this.#counters.get(keyId);

		if (state === undefined) {
			return;
		}

		const replayed =
			this.#mode === "live"
				? state.liveMaxCtr !== undefined &&
					ctr <= state.liveMaxCtr
				: state.playbackCtr !== undefined &&
					ctr <= state.playbackCtr;

		if (replayed) {
			throw new Error(
				"replay detected, ctr lower than previous high watermark",
			);
		}
	}

	#recordCounter(keyId: number, ctr: bigint): void {
		let state = this.#counters.get(keyId);

		if (state === undefined) {
			state = {};
			this.#counters.set(keyId, state);
		}

		if (this.#mode === "live") {
			if (
				state.liveMaxCtr === undefined ||
				ctr > state.liveMaxCtr
			) {
				state.liveMaxCtr = ctr;
			}
		} else {
			state.playbackCtr = ctr;
		}
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
			 * Parse the unencrypted frame header first so replay state can
			 * be checked before decryption.
			 */
			const frame = Frame.parse(ciphertext);
			const keyId = frame.header.keyId;
			const ctr = BigInt(frame.header.ctr);

			/*
			 * Do not modify replay state during this check.
			 */
			this.#checkCounter(keyId, ctr);

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
				this.algorithm,
			);

			/*
			 * Commit the lease and counter only after successful decryption.
			 */
			this.#leaseRemaining = lease.remaining;
			this.#recordCounter(keyId, ctr);

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
