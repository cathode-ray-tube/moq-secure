// src/crypto.ts
import { chacha20poly1305 } from "@noble/ciphers/chacha";
import { sha256 } from "@noble/hashes/sha256";

import { deriveNonce12 } from "./nonce.js";
import { MoqSecureError } from "./errors.js";

export const AEAD_TAG_LEN = 16;

function splitCiphertextAndTag(
  combined: Uint8Array,
): { ciphertext: Uint8Array; tag: Uint8Array } {
  if (combined.length < AEAD_TAG_LEN) {
    throw new MoqSecureError("ciphertext too short for AEAD tag");
  }

  const split = combined.length - AEAD_TAG_LEN;

  return {
    ciphertext: combined.slice(0, split),
    tag: combined.slice(split),
  };
}

function combineCiphertextAndTag(
  ciphertext: Uint8Array,
  tag: Uint8Array,
): Uint8Array {
  if (tag.length !== AEAD_TAG_LEN) {
    throw new MoqSecureError("ciphertext too short for AEAD tag");
  }

  const combined = new Uint8Array(ciphertext.length + tag.length);
  combined.set(ciphertext, 0);
  combined.set(tag, ciphertext.length);
  return combined;
}

export function sha256Digest(data: Uint8Array): Uint8Array {
  return sha256(data);
}

export function aeadEncrypt(
  key: Uint8Array,
  keyId: number,
  ctr: bigint,
  aad: Uint8Array,
  plaintext: Uint8Array,
): { ciphertext: Uint8Array; tag: Uint8Array } {
  if (key.length !== 32) {
    throw new RangeError("AEAD keys must be exactly 32 bytes");
  }

  const nonce = deriveNonce12(keyId, ctr);
  const cipher = chacha20poly1305(key, nonce, aad);
  const combined = cipher.encrypt(plaintext);

  return splitCiphertextAndTag(combined);
}

export function aeadDecrypt(
  key: Uint8Array,
  keyId: number,
  ctr: bigint,
  aad: Uint8Array,
  ciphertext: Uint8Array,
  tag: Uint8Array,
): Uint8Array {
  if (key.length !== 32) {
    throw new RangeError("AEAD keys must be exactly 32 bytes");
  }

  const nonce = deriveNonce12(keyId, ctr);
  const combined = combineCiphertextAndTag(ciphertext, tag);

  try {
    const cipher = chacha20poly1305(key, nonce, aad);
    return cipher.decrypt(combined);
  } catch {
    throw new MoqSecureError("AEAD authentication failed");
  }
}
