import { PAD_LEN_FIELD_LEN } from "./constants.js";

export function addPadding(
  plaintext: Uint8Array,
  padLength: number,
): Uint8Array {
  if (
    !Number.isSafeInteger(padLength) ||
    padLength < 0 ||
    padLength > 0xffff_ffff
  ) {
    throw new RangeError("padLength must be a non-negative u32");
  }

  const result = new Uint8Array(
    PAD_LEN_FIELD_LEN + padLength + plaintext.length,
  );

  const view = new DataView(result.buffer);
  view.setUint32(0, padLength, false);

  // The zero-initialized region between the length field and plaintext
  // is the padding.
  result.set(plaintext, PAD_LEN_FIELD_LEN + padLength);

  return result;
}

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
  const contentStart = PAD_LEN_FIELD_LEN + padLength;

  if (contentStart > paddedPlaintext.length) {
    throw new RangeError("invalid pad length");
  }

  return paddedPlaintext.slice(contentStart);
}
