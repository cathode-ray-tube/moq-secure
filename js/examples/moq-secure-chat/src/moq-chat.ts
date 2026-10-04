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
    const relayUrl = parseRelayUrl(this.relayUrl);

    console.log(
      "Connecting to MoQ relay:",
      relayUrl.href,
    );

    this.#connection = await Moq.Connection.connect(
      relayUrl,
      {
        publish: this.#broadcast.consume(),
      },
    );

    console.log("Connected to MoQ relay");
  }

  async send(message: ChatMessage): Promise<void> {
    if (!this.#connection) {
      throw new Error("Publisher is not connected");
    }

    const payload = await this.codec.encrypt(message);

    if (!this.#group) {
      this.#group = this.#track.appendGroup();
    }

    this.#group.writeFrame({
      payload,
    });
  }

  close(): void {
    this.#group?.close();
    this.#broadcast.close();
    this.#connection?.close();

    this.#group = undefined;
    this.#connection = undefined;
  }
}

export class MoqChatSubscription {
  readonly #seen = new Set<string>();

  #connection?: Connection;

  constructor(
    private readonly relayUrl: string,
    private readonly broadcastName: string,
    private readonly codec: SecureChatCodec,
    private readonly onMessage: (
      message: ChatMessage,
    ) => void,
  ) {}

  async connect(): Promise<void> {
    const relayUrl = parseRelayUrl(this.relayUrl);

    console.log(
      "Connecting to MoQ relay:",
      relayUrl.href,
    );

    this.#connection = await Moq.Connection.connect(
      relayUrl,
    );

    const subscription = this.#connection
      .consume(Moq.Path.from(this.broadcastName))
      .track(CHAT_TRACK)
      .subscribe({
        priority: 0,
      });

    console.log("Connected to MoQ relay");

    void this.readLoop(subscription);
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
        try {
          const frame = await group.readFrame();

          if (!frame) {
            break;
          }

          const message = await this.codec.decrypt(
            frame.payload,
          );

          if (this.#seen.has(message.messageId)) {
            continue;
          }

          this.#seen.add(message.messageId);
          this.onMessage(message);
        } catch (error) {
          if (error instanceof Moq.RemoteError) {
            console.warn(
              "Chat group reset:",
              error,
            );

            break;
          }

          console.error(
            "Unable to decrypt chat message:",
            error,
          );

          break;
        }
      }
    }
  }

  close(): void {
    this.#connection?.close();
    this.#connection = undefined;
  }
}
