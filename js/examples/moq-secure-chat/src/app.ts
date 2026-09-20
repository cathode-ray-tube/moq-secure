import { encodeBase64 } from "./base64.ts";
import {
  MoqChatPublisher,
  MoqChatSubscription,
} from "./moq-chat.ts";
import { secureFactory } from "./secure-factory.ts";
import type {
  ChatMessage,
  Identity,
  SubscriptionConfig,
} from "./types.ts";

class MoqSecureChat extends HTMLElement {
  #identity?: Identity;
  #publisher?: MoqChatPublisher;
  #subscriptions = new Map<
    string,
    MoqChatSubscription
  >();

  connectedCallback(): void {
    this.render();
    this.bindEvents();
  }

  private render(): void {
    this.innerHTML = `
      <style>
        :host {
          display: block;
          min-height: 100vh;
          color: #eef2ff;
          background:
            radial-gradient(
              circle at 10% 0%,
              #334b9b 0,
              #131a36 35%,
              #070a14 100%
            );
          font-family: Inter, system-ui, sans-serif;
        }

        * {
          box-sizing: border-box;
        }

        .app {
          width: min(1180px, calc(100% - 32px));
          margin: auto;
          padding: 32px 0;
        }

        header {
          margin-bottom: 22px;
        }

        h1 {
          margin: 0;
          font-size: clamp(2.4rem, 7vw, 5.4rem);
          letter-spacing: -0.08em;
        }

        h2 {
          margin-top: 0;
          letter-spacing: -0.03em;
        }

        .muted {
          color: #aeb9dc;
        }

        .grid {
          display: grid;
          grid-template-columns: 370px 1fr;
          gap: 18px;
        }

        .card {
          padding: 20px;
          border: 1px solid #ffffff1c;
          border-radius: 24px;
          background: #10172bd9;
          box-shadow: 0 24px 90px #0005;
          backdrop-filter: blur(18px);
        }

        .card + .card {
          margin-top: 18px;
        }

        label {
          display: grid;
          gap: 7px;
          margin-top: 14px;
          color: #b9c6e8;
          font-size: .84rem;
        }

        input, textarea {
          width: 100%;
          border: 1px solid #ffffff20;
          border-radius: 12px;
          padding: 11px;
          color: white;
          background: #070b17cc;
          font: inherit;
        }

        textarea {
          min-height: 76px;
          resize: vertical;
        }

        button {
          border: 0;
          border-radius: 12px;
          padding: 11px 15px;
          color: white;
          background: linear-gradient(135deg, #657aff, #ad64ff);
          font-weight: 750;
          cursor: pointer;
        }

        button.secondary {
          background: #ffffff16;
        }

        .actions {
          display: flex;
          gap: 10px;
          flex-wrap: wrap;
          margin-top: 16px;
        }

        #messages {
          height: 540px;
          overflow: auto;
          padding: 4px;
        }

        .message {
          max-width: 76%;
          margin: 10px 0;
          padding: 12px 14px;
          border-radius: 16px;
          background: #ffffff0d;
        }

        .mine {
          margin-left: auto;
          background: linear-gradient(135deg, #3e52ba, #713e9e);
        }

        .meta {
          margin-bottom: 5px;
          color: #aab7dc;
          font-size: .75rem;
        }

        form.composer {
          display: flex;
          gap: 10px;
          margin-top: 16px;
        }

        .composer input {
          flex: 1;
        }

        @media (max-width: 820px) {
          .grid {
            grid-template-columns: 1fr;
          }
        }
      </style>

      <div class="app">
        <header>
          <div class="muted">Encrypted, signed messaging over MoQ</div>
          <h1>Secure Chat</h1>
        </header>

        <div class="grid">
          <aside>
            <section class="card">
              <h2>Your identity</h2>

              <label>
                Display name
                <input id="display-name" autocomplete="off">
              </label>

              <label>
                Encryption key
                <input id="encryption-key" readonly>
              </label>

              <label>
                Signing private key
                <textarea id="signing-private-key" readonly></textarea>
              </label>

              <label>
                Signing public key
                <textarea id="signing-public-key" readonly></textarea>
              </label>

              <div class="actions">
                <button id="generate">
                  Generate keys
                </button>
              </div>
            </section>

            <section class="card">
              <h2>Publish</h2>

              <label>
                Relay URL
                <input id="publish-relay"
                  placeholder="https://relay.example/anon">
              </label>

              <label>
                Broadcast name
                <input id="publish-name"
                  placeholder="room/alice">
              </label>

              <div class="actions">
                <button id="start-publishing">
                  Start publishing
                </button>
              </div>
            </section>

            <section class="card">
              <h2>Add subscription</h2>

              <label>
                Relay URL
                <input id="subscribe-relay"
                  placeholder="https://relay.example/anon">
              </label>

              <label>
                Broadcast name
                <input id="subscribe-name"
                  placeholder="room/alice">
              </label>

              <label>
                Encryption key
                <input id="remote-encryption-key">
              </label>

              <label>
                Broadcaster signing public key
                <textarea id="remote-public-key"></textarea>
              </label>

              <div class="actions">
                <button id="subscribe">
                  Add subscription
                </button>
              </div>
            </section>
          </aside>

          <main class="card">
            <div id="messages"></div>

            <form id="composer" class="composer">
              <input id="message"
                placeholder="Write an encrypted message..."
                autocomplete="off">
              <button>Send</button>
            </form>
          </main>
        </div>
      </div>
    `;
  }

  private bindEvents(): void {
    this.button("generate").addEventListener(
      "click",
      () => void this.generate(),
    );

    this.button("start-publishing").addEventListener(
      "click",
      () => void this.startPublishing(),
    );

    this.button("subscribe").addEventListener(
      "click",
      () => void this.addSubscription(),
    );

    this.form("composer").addEventListener(
      "submit",
      (event) => {
        event.preventDefault();
        void this.send();
      },
    );
  }

  private async generate(): Promise<void> {
    this.#identity =
      await secureFactory.generateIdentity();

    this.input("encryption-key").value =
      encodeBase64(this.#identity.encryptionKey);

    this.textarea("signing-private-key").value =
      encodeBase64(
        this.#identity.signingPrivateKey,
      );

    this.textarea("signing-public-key").value =
      encodeBase64(
        this.#identity.signingPublicKey,
      );
  }

  private async startPublishing(): Promise<void> {
    if (!this.#identity) {
      throw new Error("Generate keys first");
    }

    const relayUrl = this.input(
      "publish-relay",
    ).value.trim();

    const broadcastName = this.input(
      "publish-name",
    ).value.trim();

    const codec =
      await secureFactory.createPublisherCodec(
        this.#identity,
      );

    this.#publisher = new MoqChatPublisher(
      relayUrl,
      broadcastName,
      codec,
    );

    await this.#publisher.connect();
  }

  private async addSubscription(): Promise<void> {
    const config: SubscriptionConfig = {
      id: crypto.randomUUID(),
      relayUrl: this.input(
        "subscribe-relay",
      ).value.trim(),
      broadcastName: this.input(
        "subscribe-name",
      ).value.trim(),
      encryptionKey: this.decode(
        this.input("remote-encryption-key").value,
      ),
      broadcasterPublicKey: this.decode(
        this.textarea("remote-public-key").value,
      ),
    };

    const codec =
      await secureFactory.createSubscriberCodec(
        config.encryptionKey,
        config.broadcasterPublicKey,
      );

    const subscription =
      new MoqChatSubscription(
        config.relayUrl,
        config.broadcastName,
        codec,
        (message) =>
          this.addMessage(message, false),
      );

    this.#subscriptions.set(
      config.id,
      subscription,
    );

    await subscription.connect();
  }

  private async send(): Promise<void> {
    if (!this.#publisher) {
      throw new Error(
        "Start publishing before sending",
      );
    }

    const input = this.input("message");
    const body = input.value.trim();

    if (!body) {
      return;
    }

    const message: ChatMessage = {
      version: 1,
      messageId: crypto.randomUUID(),
      sender:
        this.input("display-name").value.trim() ||
        "anonymous",
      body,
      createdAt: Date.now(),
    };

    await this.#publisher.send(message);
    this.addMessage(message, true);
    input.value = "";
  }

  private addMessage(
    message: ChatMessage,
    mine: boolean,
  ): void {
    const list = this.element("messages");
    const item = document.createElement("article");

    item.className = mine
      ? "message mine"
      : "message";

    const meta = document.createElement("div");
    meta.className = "meta";
    meta.textContent = message.sender;

    const body = document.createElement("div");
    body.textContent = message.body;

    item.append(meta, body);
    list.append(item);
    list.scrollTop = list.scrollHeight;
  }

  private decode(value: string): Uint8Array {
    const binary = atob(value.trim());

    return Uint8Array.from(binary, (char) =>
      char.charCodeAt(0),
    );
  }

  private element(id: string): HTMLElement {
    const element = this.querySelector(
      `#${id}`,
    );

    if (!element) {
      throw new Error(`Missing #${id}`);
    }

    return element as HTMLElement;
  }

  private input(id: string): HTMLInputElement {
    return this.element(id) as HTMLInputElement;
  }

  private textarea(id: string): HTMLTextAreaElement {
    return this.element(id) as HTMLTextAreaElement;
  }

  private button(id: string): HTMLButtonElement {
    return this.element(id) as HTMLButtonElement;
  }

  private form(id: string): HTMLFormElement {
    return this.element(id) as HTMLFormElement;
  }
}

customElements.define(
  "moq-secure-chat",
  MoqSecureChat,
);
