import { chacha20poly1305 } from "@noble/ciphers/chacha.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { AEAD_TAG_LEN } from "./constants.js";
import { deriveNonce12 } from "./nonce.js";
import { MoqSecureError } from "./errors.js";

export type AeadAlgorithm = "CHACHA20-POLY1305" | "AES-256-GCM";

function getWebCrypto(): Crypto {
  const cryptoObject = globalThis.crypto;

  if (!cryptoObject?.subtle) {
    throw new Error("WebCrypto API is not available");
  }

  return cryptoObject;
}

function toWebCryptoBuffer(data: Uint8Array): Uint8Array<ArrayBuffer> {
  const copy = new Uint8Array(data.byteLength);
  copy.set(data);
  return copy;
}

function asUint8Array(data: ArrayBuffer): Uint8Array {
  return new Uint8Array(data);
}

export function sha256Digest(data: Uint8Array): Uint8Array {
  return sha256(data);
}

function validateAeadInputs(key: Uint8Array, tag?: Uint8Array): void {
  if (key.length !== 32) {
    throw new RangeError("AEAD key must be 32 bytes");
  }

  if (tag !== undefined && tag.length !== AEAD_TAG_LEN) {
    throw MoqSecureError.authFailed();
  }
}

function encryptChaCha20Poly1305(
  key: Uint8Array,
  keyId: number,
  ctr: bigint,
  aad: Uint8Array,
  plaintext: Uint8Array,
): { ciphertext: Uint8Array; tag: Uint8Array } {
  const combined = chacha20poly1305(
    key,
    deriveNonce12(keyId, ctr),
    aad,
  ).encrypt(plaintext);

  return {
    ciphertext: combined.slice(0, combined.length - AEAD_TAG_LEN),
    tag: combined.slice(combined.length - AEAD_TAG_LEN),
  };
}

async function encryptAes256Gcm(
  key: Uint8Array,
  keyId: number,
  ctr: bigint,
  aad: Uint8Array,
  plaintext: Uint8Array,
): Promise<{ ciphertext: Uint8Array; tag: Uint8Array }> {
  const cryptoObject = getWebCrypto();

  const cryptoKey = await cryptoObject.subtle.importKey(
    "raw",
    toWebCryptoBuffer(key),
    { name: "AES-GCM" },
    false,
    ["encrypt"],
  );

  const combined = await cryptoObject.subtle.encrypt(
    {
      name: "AES-GCM",
      iv: toWebCryptoBuffer(deriveNonce12(keyId, ctr)),
      additionalData: toWebCryptoBuffer(aad),
      tagLength: AEAD_TAG_LEN * 8,
    },
    cryptoKey,
    toWebCryptoBuffer(plaintext),
  );

  const combinedBytes = asUint8Array(combined);

  return {
    ciphertext: combinedBytes.slice(
      0,
      combinedBytes.length - AEAD_TAG_LEN,
    ),
    tag: combinedBytes.slice(combinedBytes.length - AEAD_TAG_LEN),
  };
}

export async function aeadEncrypt(
  algorithm: AeadAlgorithm,
  key: Uint8Array,
  keyId: number,
  ctr: bigint,
  aad: Uint8Array,
  plaintext: Uint8Array,
): Promise<{ ciphertext: Uint8Array; tag: Uint8Array }> {
  validateAeadInputs(key);

  switch (algorithm) {
    case "CHACHA20-POLY1305":
      return encryptChaCha20Poly1305(key, keyId, ctr, aad, plaintext);

    case "AES-256-GCM":
      return encryptAes256Gcm(key, keyId, ctr, aad, plaintext);

    default:
      throw new RangeError(`Unsupported AEAD algorithm: ${algorithm}`);
  }
}

function decryptChaCha20Poly1305(
  key: Uint8Array,
  keyId: number,
  ctr: bigint,
  aad: Uint8Array,
  ciphertext: Uint8Array,
  tag: Uint8Array,
): Uint8Array {
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

async function decryptAes256Gcm(
  key: Uint8Array,
  keyId: number,
  ctr: bigint,
  aad: Uint8Array,
  ciphertext: Uint8Array,
  tag: Uint8Array,
): Promise<Uint8Array> {
  const cryptoObject = getWebCrypto();

  const combined = new Uint8Array(ciphertext.length + tag.length);
  combined.set(ciphertext);
  combined.set(tag, ciphertext.length);

  try {
    const cryptoKey = await cryptoObject.subtle.importKey(
      "raw",
      toWebCryptoBuffer(key),
      { name: "AES-GCM" },
      false,
      ["decrypt"],
    );

    const plaintext = await cryptoObject.subtle.decrypt(
      {
        name: "AES-GCM",
        iv: toWebCryptoBuffer(deriveNonce12(keyId, ctr)),
        additionalData: toWebCryptoBuffer(aad),
        tagLength: AEAD_TAG_LEN * 8,
      },
      cryptoKey,
      toWebCryptoBuffer(combined),
    );

    return asUint8Array(plaintext);
  } catch {
    throw MoqSecureError.authFailed();
  }
}

export async function aeadDecrypt(
  algorithm: AeadAlgorithm,
  key: Uint8Array,
  keyId: number,
  ctr: bigint,
  aad: Uint8Array,
  ciphertext: Uint8Array,
  tag: Uint8Array,
): Promise<Uint8Array> {
  validateAeadInputs(key, tag);

  switch (algorithm) {
    case "CHACHA20-POLY1305":
      return decryptChaCha20Poly1305(
        key,
        keyId,
        ctr,
        aad,
        ciphertext,
        tag,
      );

    case "AES-256-GCM":
      return decryptAes256Gcm(
        key,
        keyId,
        ctr,
        aad,
        ciphertext,
        tag,
      );

    default:
      throw MoqSecureError.authFailed();
  }
}
