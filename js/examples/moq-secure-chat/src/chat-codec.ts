import type { ChatMessage } from "./types.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function encodeChatMessage(
  message: ChatMessage,
): Uint8Array {
  return encoder.encode(JSON.stringify(message));
}

export function decodeChatMessage(
  bytes: Uint8Array,
): ChatMessage {
  const value: unknown = JSON.parse(decoder.decode(bytes));

  if (!isChatMessage(value)) {
    throw new Error("Invalid chat message");
  }

  return value;
}

function isChatMessage(
  value: unknown,
): value is ChatMessage {
  if (
    typeof value !== "object" ||
    value === null
  ) {
    return false;
  }

  const object = value as Record<string, unknown>;

  return (
    object.version === 1 &&
    typeof object.messageId === "string" &&
    typeof object.sender === "string" &&
    typeof object.body === "string" &&
    typeof object.createdAt === "number"
  );
}
