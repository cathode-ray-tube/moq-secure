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
 *   zero padding || plaintext
 *
 * The pad length field is added separately by the wire layer.
 */
export function prependZeroPadding(
  plaintext: Uint8Array,
  padLength: number,
): Uint8Array {
  validatePadLength(padLength);

  const result = new Uint8Array(
    padLength + plaintext.length,
  );

  // Uint8Array is zero-initialized.
  result.set(plaintext, padLength);

  return result;
}

/**
 * Removes:
 *
 *   zero padding
 *
 * and returns the original plaintext.
 */
export function removeZeroPadding(
  paddedPlaintext: Uint8Array,
  padLength: number,
): Uint8Array {
  validatePadLength(padLength);

  if (padLength > paddedPlaintext.length) {
    throw new RangeError("plaintext is shorter than padLength");
  }

  return paddedPlaintext.slice(padLength);
}

/**
 * Removes:
 *
 *   uint32be(padLength) || zero padding
 *
 * and returns the original plaintext.
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

  return removeZeroPadding(
    paddedPlaintext.subarray(PAD_LEN_FIELD_LEN),
    padLength,
  );
}

/**
 * Backwards-compatible name used by existing callers.
 */
export const addPadding = prependZeroPadding;
