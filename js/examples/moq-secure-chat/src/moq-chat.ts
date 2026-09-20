import * as Moq from "@moq/net";
import { SecureChatCodec } from "./secure-chat.ts";
import type { ChatMessage } from "./types.ts";

const CHAT_TRACK = "messages";

export class MoqChatPublisher {
  readonly #origin = new Moq.Origin.Producer();
  readonly #broadcast: ReturnType<
    Moq.Origin.Producer["createBroadcast"]
  >;
  readonly #track: ReturnType<
    ReturnType<
      Moq.Origin.Producer["createBroadcast"]
    >["createTrack"]
  >;

  #connection?: Awaited<
    ReturnType<typeof Moq.Connection.connect>
  >;

  #group?: ReturnType<
    ReturnType<
      ReturnType<
        Moq.Origin.Producer["createBroadcast"]
      >["createTrack"]
    >["appendGroup"]
  >;

  constructor(
    private readonly relayUrl: string,
    private readonly broadcastName: string,
    private readonly codec: SecureChatCodec,
  ) {
    this.#broadcast = this.#origin.createBroadcast(
      Moq.Path.from(this.broadcastName),
    );

    this.#track = this.#broadcast.createTrack(
      CHAT_TRACK,
    );
  }

  async connect(): Promise<void> {
    this.#connection = await Moq.Connection.connect(
      new URL(this.relayUrl),
      {
        publish: this.#origin.consume(),
      },
    );

    this.#broadcast.announce();
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
    this.#connection?.close();
  }
}

export class MoqChatSubscription {
  readonly #seen = new Set<string>();

  #connection?: Awaited<
    ReturnType<typeof Moq.Connection.connect>
  >;

  constructor(
    private readonly relayUrl: string,
    private readonly broadcastName: string,
    private readonly codec: SecureChatCodec,
    private readonly onMessage: (
      message: ChatMessage,
    ) => void,
  ) {}

  async connect(): Promise<void> {
    this.#connection = await Moq.Connection.connect(
      new URL(this.relayUrl),
    );

    const consumer = this.#connection
      .consume(Moq.Path.from(this.broadcastName))
      .track(CHAT_TRACK)
      .subscribe({
        priority: 0,
      });

    void this.readLoop(consumer);
  }

  private async readLoop(
    consumer: ReturnType<
      ReturnType<
        Awaited<
          ReturnType<typeof Moq.Connection.connect>
        >["consume"]
      >["track"]
    >["subscribe"],
  ): Promise<void> {
    for (;;) {
      const group = await consumer.recvGroup();

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
          if (error instanceof Moq.StreamError) {
            console.warn(
              "Chat group reset:",
              error.code,
            );
            break;
          }

          console.error(
            "Unable to decrypt chat message",
            error,
          );
          break;
        }
      }
    }
  }

  close(): void {
    this.#connection?.close();
  }
}
