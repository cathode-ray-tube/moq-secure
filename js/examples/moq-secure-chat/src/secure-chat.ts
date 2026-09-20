import {
  FrameDecrypter,
} from "../../../src/decrypter.js";

import {
  FrameEncrypter,
} from "../../../src/encrypter.js";
import {
  decodeChatMessage,
  encodeChatMessage,
} from "./chat-codec.ts";
import type { ChatMessage } from "./types.ts";

export class SecureChatCodec {
  constructor(
    private readonly encrypter?: FrameEncrypter,
    private readonly decrypter?: FrameDecrypter,
  ) {}

  async encrypt(
    message: ChatMessage,
  ): Promise<Uint8Array> {
    if (!this.encrypter) {
      throw new Error("Chat encrypter is not configured");
    }

    const plaintext = encodeChatMessage(message);

    // The sequence number is intentionally 0.
    return this.encrypter.encrypt(0, plaintext);
  }

  async decrypt(
    ciphertext: Uint8Array,
  ): Promise<ChatMessage> {
    if (!this.decrypter) {
      throw new Error("Chat decrypter is not configured");
    }

    // The sequence number is intentionally 0.
    const plaintext = await this.decrypter.decrypt(
      0,
      ciphertext,
    );

    return decodeChatMessage(plaintext);
  }
}
