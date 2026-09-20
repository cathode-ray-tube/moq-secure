import {
  MoqSecureDecrypter,
} from "./decrypter.js";

import {
  MoqSecureEncrypter,
} from "./encrypter.js";

import {
  InMemoryKeyStore,
} from "../secure/keys.js";

import * as ed25519 from "@noble/ed25519";

import type { Identity } from "./types.ts";
import { SecureChatCodec } from "./secure-chat.ts";

async function generateSigningKeyPair(): Promise<{
  privateKey: Uint8Array;
  publicKey: Uint8Array;
}> {
  // Ed25519 private keys used by @noble/ed25519 are 32-byte seeds.
  const privateKey = crypto.getRandomValues(
    new Uint8Array(32),
  );

  const publicKey = await ed25519.getPublicKeyAsync(privateKey);

  return {
    privateKey,
    publicKey,
  };
}

export interface SecureFactory {
  createPublisherCodec(
    identity: Identity,
  ): Promise<SecureChatCodec>;

  createSubscriberCodec(
    encryptionKey: Uint8Array,
    broadcasterPublicKey: Uint8Array,
  ): Promise<SecureChatCodec>;

  generateIdentity(): Promise<Identity>;
}

export const secureFactory: SecureFactory = {
  async generateIdentity(): Promise<Identity> {
    const encryptionKey = crypto.getRandomValues(
      new Uint8Array(32),
    );

    const signing = await generateSigningKeyPair();

    return {
      displayName: "",
      encryptionKey,
      signingPrivateKey: signing.privateKey,
      signingPublicKey: signing.publicKey,
    };
  },

  async createPublisherCodec(
    identity: Identity,
  ): Promise<SecureChatCodec> {
    const keyStore = new InMemoryKeyStore();
    keyStore.setKey(0, identity.encryptionKey);

    const encrypter = new MoqSecureEncrypter({
      keyStore,
      signingPrivateKey: identity.signingPrivateKey,
      keyId: 0,
      nSigned: 1,
      maybeSign: false,
      padLen: 0,
    });

    return new SecureChatCodec(encrypter);
  },

  async createSubscriberCodec(
    encryptionKey: Uint8Array,
    broadcasterPublicKey: Uint8Array,
  ): Promise<SecureChatCodec> {
    const keyStore = new InMemoryKeyStore();
    keyStore.setKey(0, encryptionKey);

    const decrypter = new MoqSecureDecrypter({
      keyStore,
      broadcasterPublicKey,
    });

    return new SecureChatCodec(undefined, decrypter);
  },
};
