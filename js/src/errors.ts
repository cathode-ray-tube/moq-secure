export type MoqSecureErrorCode =
  | "InvalidMagic"
  | "UnsupportedVersion"
  | "TruncatedFrame"
  | "CiphertextTooShort"
  | "InvalidEncryptedFlag"
  | "InvalidSigFlag"
  | "AeadAuthFailed"
  | "InvalidSignature"
  | "SigningMismatch"
  | "MissingSigSlot"
  | "SignatureNotAllowedByNSigned"
  | "DecryptFailed"
  | "InvalidKeyId";

export class MoqSecureError extends Error {
  readonly code: MoqSecureErrorCode;
  readonly value?: number;

  constructor(
    code: MoqSecureErrorCode,
    message?: string,
    value?: number,
  ) {
    super(message ?? code);
    this.name = "MoqSecureError";
    this.code = code;
    this.value = value;
  }

  static invalidMagic(): MoqSecureError {
    return new MoqSecureError("InvalidMagic", "invalid magic bytes");
  }

  static unsupportedVersion(version: number): MoqSecureError {
    return new MoqSecureError(
      "UnsupportedVersion",
      `unsupported version: ${version}`,
      version,
    );
  }

  static truncated(): MoqSecureError {
    return new MoqSecureError(
      "TruncatedFrame",
      "not enough bytes in frame",
    );
  }

  static authFailed(): MoqSecureError {
    return new MoqSecureError(
      "AeadAuthFailed",
      "AEAD authentication failed",
    );
  }

  static invalidSignature(): MoqSecureError {
    return new MoqSecureError(
      "InvalidSignature",
      "signature invalid or signature verification failed",
    );
  }
}
