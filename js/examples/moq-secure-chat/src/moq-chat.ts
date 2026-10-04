import * as Moq from "@moq/net";

import { SecureChatCodec } from "./secure-chat.ts";
import type { ChatMessage } from "./types.ts";

const DEFAULT_RELAY_URL =
  "https://cdn.moqtv.com:4443/";

const CHAT_TRACK = "messages";

type Connection = Awaited<
  ReturnType<typeof Moq.Connection.connect>
>;

type Broadcast = Moq.Broadcast.Producer;
type Track = Moq.Track.Producer;
type Group = ReturnType<Track["appendGroup"]>;

type Subscription = ReturnType<
  ReturnType<
    Connection["consume"]
  >["track"]
>["subscribe"];

function parseRelayUrl(relayUrl: string): URL {
  const value = relayUrl.trim();

  if (!value) {
    throw new Error(
      "MoQ relay URL is empty. Configure a valid relay URL.",
    );
  }

  let url: URL;

  try {
    url = new URL(value);
  } catch {
    throw new Error(
      `Invalid MoQ relay URL: "${relayUrl}"`,
    );
  }

  if (
    url.protocol !== "http:" &&
    url.protocol !== "https:"
  ) {
    throw new Error(
      `Invalid relay URL protocol "${url.protocol}". ` +
        "Use http:// or https://.",
    );
  }

  return url;
}

export class MoqChatPublisher {
  readonly #broadcast: Broadcast;
  readonly #track: Track;

  readonly #relayUrl: string;
  readonly #broadcastName: string;
  readonly #codec: SecureChatCodec;

  #connection?: Connection;
  #group?: Group;
  #closed = false;

  // Existing form:
  // new MoqChatPublisher(relayUrl, broadcastName, codec)
  constructor(
    relayUrl: string,
    broadcastName: string,
    codec: SecureChatCodec,
  );

  // Default-relay form:
  // new MoqChatPublisher(broadcastName, codec)
  constructor(
    broadcastName: string,
    codec: SecureChatCodec,
  );

  constructor(
    relayOrBroadcast: string,
    broadcastOrCodec: string | SecureChatCodec,
    maybeCodec?: SecureChatCodec,
  ) {
    if (
      typeof broadcastOrCodec === "string" &&
      maybeCodec
    ) {
      this.#relayUrl = relayOrBroadcast;
      this.#broadcastName = broadcastOrCodec;
      this.#codec = maybeCodec;
    } else {
      this.#relayUrl = DEFAULT_RELAY_URL;
      this.#broadcastName = relayOrBroadcast;
      this.#codec =
        broadcastOrCodec as SecureChatCodec;
    }

    this.#broadcast = new Moq.Broadcast.Producer(
      Moq.Path.from(this.#broadcastName),
    );

    this.#track = this.#broadcast.createTrack(
      CHAT_TRACK,
    );
  }

  async connect(): Promise<void> {
    if (this.#closed) {
      throw new Error("Publisher is closed");
    }

    if (this.#connection) {
      return;
    }

    const relayUrl = parseRelayUrl(this.#relayUrl);

    console.log(
      "Connecting to MoQ relay:",
      relayUrl.href,
    );

    const connection = await Moq.Connection.connect(
      relayUrl,
      {
        publish: this.#broadcast.consume(),
      },
    );

    if (this.#closed) {
      connection.close();
      return;
    }

    this.#connection = connection;

    console.log("Connected to MoQ relay", {
      broadcast: this.#broadcastName,
      track: CHAT_TRACK,
    });
  }

  async send(message: ChatMessage): Promise<void> {
    if (this.#closed) {
      throw new Error("Publisher is closed");
    }

    if (!this.#connection) {
      throw new Error("Publisher is not connected");
    }

    const payload = await this.#codec.encrypt(
      message,
    );

    if (!this.#group) {
      this.#group = this.#track.appendGroup();
    }

    this.#group.writeFrame({
      payload,
    });
  }

  close(): void {
    if (this.#closed) {
      return;
    }

    this.#closed = true;

    this.#group?.close();
    this.#group = undefined;

    this.#broadcast.close();

    this.#connection?.close();
    this.#connection = undefined;
  }
}

export class MoqChatSubscription {
  readonly #seen = new Set<string>();

  readonly #relayUrl: string;
  readonly #broadcastName: string;
  readonly #codec: SecureChatCodec;
  readonly #onMessage: (
    message: ChatMessage,
  ) => void;

  #connection?: Connection;
  #closed = false;

  // Existing form:
  // new MoqChatSubscription(
  //   relayUrl,
  //   broadcastName,
  //   codec,
  //   onMessage,
  // )
  constructor(
    relayUrl: string,
    broadcastName: string,
    codec: SecureChatCodec,
    onMessage: (message: ChatMessage) => void,
  );

  // Default-relay form:
  // new MoqChatSubscription(
  //   broadcastName,
  //   codec,
  //   onMessage,
  // )
  constructor(
    broadcastName: string,
    codec: SecureChatCodec,
    onMessage: (message: ChatMessage) => void,
  );

  constructor(
    relayOrBroadcast: string,
    broadcastOrCodec: string | SecureChatCodec,
    codecOrOnMessage:
      | SecureChatCodec
      | ((message: ChatMessage) => void),
    maybeOnMessage?: (
      message: ChatMessage,
    ) => void,
  ) {
    if (
      typeof broadcastOrCodec === "string" &&
      typeof codecOrOnMessage !== "function" &&
      maybeOnMessage
    ) {
      this.#relayUrl = relayOrBroadcast;
      this.#broadcastName = broadcastOrCodec;
      this.#codec =
        codecOrOnMessage as SecureChatCodec;
      this.#onMessage = maybeOnMessage;
    } else {
      this.#relayUrl = DEFAULT_RELAY_URL;
      this.#broadcastName = relayOrBroadcast;
      this.#codec =
        broadcastOrCodec as SecureChatCodec;
      this.#onMessage =
        codecOrOnMessage as (
          message: ChatMessage,
        ) => void;
    }
  }

  async connect(): Promise<void> {
    if (this.#closed) {
      throw new Error("Subscription is closed");
    }

    if (this.#connection) {
      return;
    }

    const relayUrl = parseRelayUrl(this.#relayUrl);

    console.log(
      "Connecting to MoQ relay:",
      relayUrl.href,
    );

    const connection = await Moq.Connection.connect(
      relayUrl,
    );

    if (this.#closed) {
      connection.close();
      return;
    }

    this.#connection = connection;

    const subscription = connection
      .consume(Moq.Path.from(this.#broadcastName))
      .track(CHAT_TRACK)
      .subscribe({
        priority: 0,
      });

    console.log("Connected to MoQ relay", {
      broadcast: this.#broadcastName,
      track: CHAT_TRACK,
    });

    void this.readLoop(subscription).catch(
      (error) => {
        if (!this.#closed) {
          console.error(
            "MoQ chat subscription stopped:",
            error,
          );
        }
      },
    );
  }

  private async readLoop(
    subscription: Subscription,
  ): Promise<void> {
    for (;;) {
      const group = await subscription.recvGroup();

      if (!group) {
        return;
      }

      for (;;) {
        let frame;

        try {
          frame = await group.readFrame();
        } catch (error) {
          if (error instanceof Moq.RemoteError) {
            console.warn(
              "Chat group reset:",
              error,
            );
            break;
          }

          throw error;
        }

        if (!frame) {
          break;
        }

        try {
          const message = await this.#codec.decrypt(
            frame.payload,
          );

          if (this.#seen.has(message.messageId)) {
            continue;
          }

          this.#seen.add(message.messageId);
          this.#onMessage(message);
        } catch (error) {
          console.error(
            "Unable to decrypt chat message:",
            error,
          );
        }
      }
    }
  }

  close(): void {
    if (this.#closed) {
      return;
    }

    this.#closed = true;

    this.#connection?.close();
    this.#connection = undefined;
  }
}
