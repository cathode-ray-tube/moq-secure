export const MAGIC = new Uint8Array([
  0x4d,
  0x4f,
  0x51,
  0x53,
]); // MOQS

export const VERSION = 1;

export const SIG_SLOT_LEN = 64;
export const AEAD_TAG_LEN = 16;
export const PAD_LEN_FIELD_LEN = 4;
export const FIXED_HEADER_LEN = 17;

/**
 * Encryption algorithm identifiers used in the wire header.
 *
 * These numeric values are part of the protocol and must remain stable.
 */
export const EncryptionType = {
  NONE: 0,
  CHACHA20_POLY1305: 1,
  AES_256_GCM: 2,
} as const;

export type EncryptionType =
  (typeof EncryptionType)[keyof typeof EncryptionType];
