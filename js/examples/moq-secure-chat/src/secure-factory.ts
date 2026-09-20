import {
  MoqSecureDecrypter,
} from "./decrypter.js";

import {
  MoqSecureEncrypter,
} from "./encrypter.js";

import type { Identity } from "./types.ts";
import { SecureChatCodec } from "./secure-chat.ts";

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

/*
 * Replace the three TODO sections below with your moq-secure APIs.
 */
export const secureFactory: SecureFactory = {
  async generateIdentity(): Promise<Identity> {
    /*
     * Example shape only:

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
    */

    throw new Error(
      "Implement generateIdentity() using moq-secure",
    );
  },

  async createPublisherCodec(
    identity: Identity,
  ): Promise<SecureChatCodec> {
    /*
     * Replace these constructors with your actual KeyStore
     * initialization code.

     const keyStore = createKeyStore(
       identity.encryptionKey,
     );

     const encrypter = new MoqSecureEncrypter({
       keyStore,
       signingPrivateKey: identity.signingPrivateKey,
       keyId: 0,
       nSigned: 1,
       maybeSign: false,
       padLen: 0,
     });

     return new SecureChatCodec(encrypter);
    */

    void identity;

    throw new Error(
      "Implement createPublisherCodec() using moq-secure",
    );
  },

  async createSubscriberCodec(
    encryptionKey: Uint8Array,
    broadcasterPublicKey: Uint8Array,
  ): Promise<SecureChatCodec> {
    /*
     * Replace these constructors with your actual KeyStore
     * initialization code.

     const keyStore = createKeyStore(encryptionKey);

     const decrypter = new MoqSecureDecrypter({
       keyStore,
       broadcasterPublicKey,
     });

     return new SecureChatCodec(undefined, decrypter);
    */

    void encryptionKey;
    void broadcasterPublicKey;

    throw new Error(
      "Implement createSubscriberCodec() using moq-secure",
    );
  },
};
