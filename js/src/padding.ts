import { PAD_LEN_FIELD_LEN } from "./constants.js";

function validatePadLength(padLength: number): void {
  if (
    !Number.isSafeInteger(padLength) ||
    padLength < 0 ||
    padLength > 0xffff_ffff
  ) {
    throw new RangeError("padLength must be a non-negative u32");
  }
}

/**
 * Produces:
 *
 *   uint32be(padLength) || zero padding || plaintext
 *
 * The returned value is the complete plaintext passed to AEAD.
 */
export function prependZeroPadding(
  plaintext: Uint8Array,
  padLength: number,
): Uint8Array {
  validatePadLength(padLength);

  const result = new Uint8Array(
    PAD_LEN_FIELD_LEN + padLength + plaintext.length,
  );

  const view = new DataView(result.buffer);
  view.setUint32(0, padLength, false);

  // The Uint8Array is zero-initialized, so this region is zero padding.
  result.set(
    plaintext,
    PAD_LEN_FIELD_LEN + padLength,
  );

  return result;
}

/**
 * Removes:
 *
 *   uint32be(padLength) || zero padding
 *
 * and returns the original plaintext.
 */
export function removeZeroPadding(
  paddedPlaintext: Uint8Array,
  padLength: number,
): Uint8Array {
  validatePadLength(padLength);

  const contentStart = PAD_LEN_FIELD_LEN + padLength;

  if (contentStart > paddedPlaintext.length) {
    throw new RangeError("plaintext is shorter than padLength");
  }

  return paddedPlaintext.slice(contentStart);
}

/**
 * Removes padding when the pad length is stored in the first four bytes.
 */
export function removePadding(
  paddedPlaintext: Uint8Array,
): Uint8Array {
  if (paddedPlaintext.length < PAD_LEN_FIELD_LEN) {
    throw new RangeError("padded plaintext is too short");
  }

  const view = new DataView(
    paddedPlaintext.buffer,
    paddedPlaintext.byteOffset,
    paddedPlaintext.byteLength,
  );

  const padLength = view.getUint32(0, false);

  return removeZeroPadding(paddedPlaintext, padLength);
}

/**
 * Backwards-compatible name used by wire.ts.
 */
export const addPadding = prependZeroPadding;
