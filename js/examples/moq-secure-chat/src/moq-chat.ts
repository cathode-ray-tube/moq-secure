import * as Moq from "@moq/net";

import { SecureChatCodec } from "./secure-chat.ts";
import type { ChatMessage } from "./types.ts";

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

  #connection?: Connection;
  #group?: Group;
  #connectPromise?: Promise<void>;
  #closed = false;

  constructor(
    private readonly relayUrl: string,
    private readonly broadcastName: string,
    private readonly codec: SecureChatCodec,
  ) {
    this.#broadcast = new Moq.Broadcast.Producer(
      Moq.Path.from(this.broadcastName),
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

    if (this.#connectPromise) {
      return this.#connectPromise;
    }

    this.#connectPromise = this.#connect();

    try {
      await this.#connectPromise;
    } finally {
      this.#connectPromise = undefined;
    }
  }

  async #connect(): Promise<void> {
    const relayUrl = parseRelayUrl(this.relayUrl);

    console.log(
      "Connecting to MoQ relay:",
      relayUrl.href,
    );

    const connection = await Moq.Connection.connect(
      relayUrl,
      {
        // This is sufficient for a Broadcast.Producer.
        // A subscriber does not need to exist first.
        publish: this.#broadcast.consume(),
      },
    );

    if (this.#closed) {
      connection.close();
      return;
    }

    this.#connection = connection;

    console.log(
      "Connected to MoQ relay",
      {
        broadcast: this.broadcastName,
        track: CHAT_TRACK,
      },
    );
  }

  async send(message: ChatMessage): Promise<void> {
    if (this.#closed) {
      throw new Error("Publisher is closed");
    }

    await this.connect();

    if (!this.#connection) {
      throw new Error("Publisher is not connected");
    }

    const payload = await this.codec.encrypt(message);

    if (!this.#group) {
      this.#group = this.#track.appendGroup();
    }

    try {
      this.#group.writeFrame({ payload });
    } catch (error) {
      this.#group = undefined;

      throw new Error(
        "Unable to publish chat message",
        { cause: error },
      );
    }
  }

  close(): void {
    if (this.#closed) {
      return;
    }

    this.#closed = true;

    this.#group?.close();
    this.#group = undefined;

    this.#connection?.close();
    this.#connection = undefined;

    this.#broadcast.close();
  }
}

export class MoqChatSubscription {
  readonly #seen = new Set<string>();

  #connection?: Connection;
  #readLoopPromise?: Promise<void>;
  #closed = false;

  constructor(
    private readonly relayUrl: string,
    private readonly broadcastName: string,
    private readonly codec: SecureChatCodec,
    private readonly onMessage: (
      message: ChatMessage,
    ) => void,
  ) {}

  async connect(): Promise<void> {
    if (this.#closed) {
      throw new Error("Subscription is closed");
    }

    if (this.#connection) {
      return;
    }

    const relayUrl = parseRelayUrl(this.relayUrl);

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
      .consume(Moq.Path.from(this.broadcastName))
      .track(CHAT_TRACK)
      .subscribe({
        priority: 0,
      });

    console.log(
      "Connected to MoQ relay",
      {
        broadcast: this.broadcastName,
        track: CHAT_TRACK,
      },
    );

    this.#readLoopPromise = this.readLoop(
      subscription,
    );

    void this.#readLoopPromise.catch((error) => {
      if (!this.#closed) {
        console.error(
          "MoQ chat subscription stopped:",
          error,
        );
      }
    });
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
          const message = await this.codec.decrypt(
            frame.payload,
          );

          if (this.#seen.has(message.messageId)) {
            continue;
          }

          this.#seen.add(message.messageId);
          this.onMessage(message);
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

