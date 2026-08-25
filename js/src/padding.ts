export function prependZeroPadding(
  plaintext: Uint8Array,
  padLength: number,
): Uint8Array {
  if (!Number.isSafeInteger(padLength) || padLength < 0) {
    throw new RangeError("padLength must be a non-negative integer");
  }

  if (padLength === 0) return plaintext.slice();

  const result = new Uint8Array(padLength + plaintext.length);
  result.set(plaintext, padLength);
  return result;
}

export function removeZeroPadding(
  plaintext: Uint8Array,
  padLength: number,
): Uint8Array {
  if (padLength < 0 || !Number.isSafeInteger(padLength)) {
    throw new RangeError("invalid padLength");
  }

  if (plaintext.length < padLength) {
    throw new RangeError("plaintext is shorter than padLength");
  }

  return plaintext.slice(padLength);
}
