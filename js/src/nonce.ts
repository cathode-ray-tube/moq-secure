// src/nonce.ts

export const NONCE_PREFIX_3 = new TextEncoder().encode("non");

export function deriveNonce12(keyId: number, ctr: bigint): Uint8Array {
  if (!Number.isInteger(keyId) || keyId < 0 || keyId > 255) {
    throw new RangeError("keyId must be an unsigned byte");
  }

  if (ctr < 0n || ctr > 0xffff_ffff_ffff_ffffn) {
    throw new RangeError("ctr must fit in an unsigned 64-bit integer");
  }

  // "non" (3 bytes) || keyId (1 byte) || ctr (8 bytes) = 12 bytes
  const nonce = new Uint8Array(12);
  nonce.set(NONCE_PREFIX_3, 0);
  nonce[3] = keyId;

  const view = new DataView(nonce.buffer);
  view.setBigUint64(4, ctr, false); // big-endian

  return nonce;
}
