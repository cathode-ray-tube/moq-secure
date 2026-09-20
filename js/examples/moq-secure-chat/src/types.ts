export interface ChatMessage {
  version: 1;
  messageId: string;
  sender: string;
  body: string;
  createdAt: number;
}

export interface Identity {
  displayName: string;
  encryptionKey: Uint8Array;
  signingPrivateKey: Uint8Array;
  signingPublicKey: Uint8Array;
}

export interface SubscriptionConfig {
  id: string;
  relayUrl: string;
  broadcastName: string;
  encryptionKey: Uint8Array;
  broadcasterPublicKey: Uint8Array;
}
