import { chacha20poly1305 } from "@noble/ciphers/chacha.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { AEAD_TAG_LEN } from "./constants.js";
import { deriveNonce12 } from "./nonce.js";
import { MoqSecureError } from "./errors.js";

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
    throw new RangeError("AEAD key must be 32 bytes");
  }

  const nonce = deriveNonce12(keyId, ctr);
  const combined = chacha20poly1305(key, nonce, aad).encrypt(plaintext);

  return {
    ciphertext: combined.slice(0, combined.length - AEAD_TAG_LEN),
    tag: combined.slice(combined.length - AEAD_TAG_LEN),
  };
}

export function aeadDecrypt(
  key: Uint8Array,
  keyId: number,
  ctr: bigint,
  aad: Uint8Array,
  ciphertext: Uint8Array,
  tag: Uint8Array,
): Uint8Array {
  if (key.length !== 32 || tag.length !== AEAD_TAG_LEN) {
    throw MoqSecureError.authFailed();
  }

  const combined = new Uint8Array(ciphertext.length + tag.length);
  combined.set(ciphertext);
  combined.set(tag, ciphertext.length);

  try {
    return chacha20poly1305(
      key,
      deriveNonce12(keyId, ctr),
      aad,
    ).decrypt(combined);
  } catch {
    throw MoqSecureError.authFailed();
  }
}
