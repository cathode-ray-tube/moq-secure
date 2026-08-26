// src/nonce.ts
import { sha256 } from "@noble/hashes/sha2.js";

export const NONCE_PREFIX_5 = new TextEncoder().encode("nonce");

export function deriveNonce12(keyId: number, ctr: bigint): Uint8Array {
  if (!Number.isInteger(keyId) || keyId < 0 || keyId > 255) {
    throw new RangeError("keyId must be an unsigned byte");
  }

  if (ctr < 0n || ctr > 0xffff_ffff_ffff_ffffn) {
    throw new RangeError("ctr must fit in an unsigned 64-bit integer");
  }

  const input = new Uint8Array(5 + 1 + 8);
  input.set(NONCE_PREFIX_5, 0);
  input[5] = keyId;

  const view = new DataView(input.buffer);
  view.setBigUint64(6, ctr, false);

  return sha256(input).slice(0, 12);
}
