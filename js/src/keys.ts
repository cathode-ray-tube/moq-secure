export interface KeyStore {
  aeadKey(keyId: number): Uint8Array | undefined;
}

export type KeyStoreErrorCode =
  | "KeyIdInvalid"
  | "KeyWrongLength"
  | "DecodeFailed"
  | "KeyNotLoaded";

export class KeyStoreError extends Error {
  readonly code: KeyStoreErrorCode;

  constructor(code: KeyStoreErrorCode, message: string) {
    super(message);
    this.name = "KeyStoreError";
    this.code = code;
  }
}

function isKeyId(value: number): boolean {
  return Number.isInteger(value) && value >= 0 && value <= 255;
}

function decodeHex(value: string): Uint8Array | undefined {
  if (value.length !== 64 || !/^[0-9a-fA-F]+$/.test(value)) {
    return undefined;
  }

  const result = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    result[i] = Number.parseInt(value.slice(i * 2, i * 2 + 2), 16);
  }
  return result;
}

function decodeBase64(value: string): Uint8Array {
  let normalized = value.replace(/-/g, "+").replace(/_/g, "/");

  while (normalized.length % 4 !== 0) normalized += "=";

  let binary: string;

  if (typeof globalThis.atob === "function") {
    binary = globalThis.atob(normalized);
  } else {
    const BufferCtor = (globalThis as any).Buffer;
    if (!BufferCtor) throw new Error("Base64 decoder unavailable");
    binary = BufferCtor.from(normalized, "base64").toString("binary");
  }

  const result = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    result[i] = binary.charCodeAt(i);
  }
  return result;
}

export class InMemoryKeyStore implements KeyStore {
  private readonly keys = new Array<Uint8Array | undefined>(256);

  aeadKey(keyId: number): Uint8Array | undefined {
    return this.keys[keyId];
  }

  setKey(keyId: number, key: Uint8Array): void {
    if (!isKeyId(keyId)) {
      throw new KeyStoreError(
        "KeyIdInvalid",
        `key_id ${keyId} must be between 0 and 255`,
      );
    }

    if (key.length !== 32) {
      throw new KeyStoreError(
        "KeyWrongLength",
        `expected 32 bytes (decoded ${key.length} bytes)`,
      );
    }

    this.keys[keyId] = key.slice();
  }

  setKeyEncoded(keyId: number, encoded: string): void {
    if (!isKeyId(keyId)) {
      throw new KeyStoreError(
        "KeyIdInvalid",
        `key_id ${keyId} must be between 0 and 255`,
      );
    }

    const value = encoded.trim();
    let decoded = decodeHex(value);

    if (!decoded) {
      try {
        decoded = decodeBase64(value);
      } catch {
        throw new KeyStoreError(
          "DecodeFailed",
          "failed to decode key as hex/base64",
        );
      }
    }

    if (decoded.length !== 32) {
      throw new KeyStoreError(
        "KeyWrongLength",
        `expected 32 bytes (decoded ${decoded.length} bytes)`,
      );
    }

    this.keys[keyId] = decoded;
  }
}
