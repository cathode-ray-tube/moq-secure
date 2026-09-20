export function encodeBase64(bytes: Uint8Array): string {
  let binary = "";

  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }

  return btoa(binary);
}

export function decodeBase64(value: string): Uint8Array {
  const binary = atob(value.trim());

  return Uint8Array.from(binary, (character) =>
    character.charCodeAt(0),
  );
}
