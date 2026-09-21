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
        @import url("https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=Space+Grotesk:wght@500;600;700&display=swap");

        :host {
          --blue-1: #031b3f;
          --blue-2: #062d63;
          --blue-3: #084d8d;
          --electric: #38d9ff;
          --electric-2: #6c7cff;
          --text: #effaff;
          --muted: #9ac3df;

          display: block;
          width: 100%;
          height: 100dvh;
          min-height: 560px;
          overflow: hidden;
          color: var(--text);
          font-family: "Inter", system-ui, sans-serif;
          background:
            radial-gradient(
              circle at 12% 5%,
              rgba(39, 174, 255, 0.7),
              transparent 29%
            ),
            radial-gradient(
              circle at 88% 85%,
              rgba(48, 91, 255, 0.48),
              transparent 34%
            ),
            linear-gradient(
              135deg,
              #06265b 0%,
              #031534 42%,
              #07508e 100%
            );
        }

        :host::before {
          content: "";
          position: fixed;
          inset: 0;
          pointer-events: none;
          opacity: 0.28;
          background-image:
            linear-gradient(
              rgba(100, 220, 255, 0.14) 1px,
              transparent 1px
            ),
            linear-gradient(
              90deg,
              rgba(100, 220, 255, 0.14) 1px,
              transparent 1px
            );
          background-size: 42px 42px;
          mask-image: linear-gradient(
            to bottom,
            black,
            transparent 95%
          );
        }

        * {
          box-sizing: border-box;
        }

        .app {
          position: relative;
          z-index: 1;
          display: flex;
          flex-direction: column;
          width: min(1280px, calc(100% - 32px));
          height: 100%;
          margin: auto;
          padding: 20px 0;
        }

        header {
          flex: 0 0 auto;
          margin-bottom: 15px;
          padding-bottom: 10px;
        }

        header::after {
          content: "";
          display: block;
          width: 100px;
          height: 3px;
          margin-top: 12px;
          border-radius: 99px;
          background: linear-gradient(
            90deg,
            var(--electric),
            var(--electric-2)
          );
          box-shadow:
            0 0 10px var(--electric),
            0 0 28px var(--electric-2);
        }

        h1,
        h2 {
          font-family: "Space Grotesk", sans-serif;
        }

        h1 {
          margin: 4px 0 0;
          color: #ffffff;
          font-size: clamp(2.7rem, 6vw, 5.2rem);
          font-weight: 700;
          letter-spacing: -0.09em;
          line-height: 0.9;
          text-shadow:
            0 0 12px rgba(56, 217, 255, 0.9),
            0 0 36px rgba(56, 217, 255, 0.4);
        }

        h2 {
          margin: 0 0 13px;
          color: #ffffff;
          font-size: 1rem;
          letter-spacing: -0.03em;
        }

        .muted {
          color: #86ebff;
          font-size: 0.68rem;
          font-weight: 800;
          letter-spacing: 0.16em;
          text-transform: uppercase;
        }

        .grid {
          display: grid;
          grid-template-columns: 350px minmax(0, 1fr);
          gap: 16px;
          min-height: 0;
          flex: 1;
        }

        aside {
          display: flex;
          flex-direction: column;
          gap: 12px;
          min-height: 0;
        }

        .card {
          position: relative;
          padding: 16px;
          overflow: hidden;
          border: 1px solid rgba(91, 229, 255, 0.7);
          border-radius: 16px;
          background:
            linear-gradient(
              145deg,
              rgba(5, 56, 109, 0.87),
              rgba(2, 24, 60, 0.88)
            );
          box-shadow:
            0 0 0 1px rgba(54, 192, 255, 0.12),
            0 0 18px rgba(31, 200, 255, 0.18),
            0 18px 48px rgba(0, 10, 35, 0.35),
            inset 0 1px rgba(255, 255, 255, 0.13);
          backdrop-filter: blur(18px);
        }

        aside .card {
          flex: 1;
          min-height: 0;
        }

        .card::before {
          content: "";
          position: absolute;
          top: 0;
          right: 16%;
          left: 16%;
          height: 2px;
          background: linear-gradient(
            90deg,
            transparent,
            var(--electric),
            transparent
          );
          box-shadow: 0 0 12px var(--electric);
        }

        main.card {
          display: flex;
          flex-direction: column;
          min-width: 0;
          min-height: 0;
          padding: 13px;
        }

        label {
          display: grid;
          gap: 5px;
          margin-top: 9px;
          color: #b8d9eb;
          font-size: 0.66rem;
          font-weight: 700;
          letter-spacing: 0.045em;
          text-transform: uppercase;
        }

        input,
        textarea {
          width: 100%;
          border: 1px solid rgba(91, 218, 255, 0.42);
          border-radius: 8px;
          padding: 8px 10px;
          outline: none;
          color: var(--text);
          background: rgba(1, 19, 47, 0.75);
          font: inherit;
          font-size: 0.8rem;
          transition:
            border-color 160ms ease,
            box-shadow 160ms ease,
            background 160ms ease;
        }

        input::placeholder,
        textarea::placeholder {
          color: #7199b7;
        }

        input:focus,
        textarea:focus {
          border-color: #9af3ff;
          background: rgba(1, 31, 68, 0.95);
          box-shadow:
            0 0 0 2px rgba(56, 217, 255, 0.18),
            0 0 15px rgba(56, 217, 255, 0.4);
        }

        input[readonly],
        textarea[readonly] {
          color: #8deaff;
          border-color: rgba(71, 220, 255, 0.52);
          background: rgba(3, 39, 75, 0.78);
        }

        textarea {
          min-height: 54px;
          resize: none;
          line-height: 1.35;
        }

        button {
          border: 1px solid rgba(169, 248, 255, 0.8);
          border-radius: 8px;
          padding: 9px 13px;
          color: #00152b;
          background: linear-gradient(
            135deg,
            #8cf4ff,
            #35d4ff 48%,
            #7888ff
          );
          box-shadow:
            0 0 10px rgba(56, 217, 255, 0.5),
            inset 0 1px rgba(255, 255, 255, 0.7);
          font: inherit;
          font-size: 0.73rem;
          font-weight: 800;
          cursor: pointer;
          transition:
            transform 160ms ease,
            filter 160ms ease,
            box-shadow 160ms ease;
        }

        button:hover {
          filter: brightness(1.15);
          transform: translateY(-2px);
          box-shadow:
            0 0 22px rgba(56, 217, 255, 0.8),
            inset 0 1px rgba(255, 255, 255, 0.8);
        }

        button:active {
          transform: translateY(0);
        }

        .actions {
          display: flex;
          gap: 8px;
          margin-top: 11px;
        }

        #messages {
          flex: 1;
          min-height: 0;
          overflow-y: auto;
          padding: 10px;
          border: 1px solid rgba(92, 220, 255, 0.42);
          border-radius: 11px;
          background:
            radial-gradient(
              circle at 50% 0%,
              rgba(33, 171, 255, 0.23),
              transparent 48%
            ),
            rgba(1, 16, 42, 0.6);
          scrollbar-color: #39d9ff transparent;
          scrollbar-width: thin;
        }

        #messages:empty::before {
          content: "Messages will appear here";
          display: grid;
          height: 100%;
          place-items: center;
          color: #87bbd7;
          font-family: "Space Grotesk", sans-serif;
          font-size: 0.85rem;
          letter-spacing: 0.05em;
        }

        .message {
          max-width: 76%;
          margin: 9px 0;
          padding: 10px 13px;
          border: 1px solid rgba(95, 218, 255, 0.42);
          border-radius: 5px 14px 14px 14px;
          color: #e9faff;
          background: rgba(8, 79, 137, 0.75);
          box-shadow:
            0 0 10px rgba(45, 190, 255, 0.16),
            0 7px 18px rgba(0, 10, 35, 0.2);
          font-size: 0.86rem;
          line-height: 1.4;
          animation: message-in 220ms ease-out;
        }

        .mine {
          margin-left: auto;
          border-color: rgba(166, 178, 255, 0.72);
          border-radius: 14px 5px 14px 14px;
          background: linear-gradient(
            135deg,
            rgba(8, 139, 180, 0.86),
            rgba(65, 70, 177, 0.88)
          );
          box-shadow:
            0 0 13px rgba(110, 134, 255, 0.28),
            0 7px 20px rgba(0, 10, 35, 0.24);
        }

        .meta {
          margin-bottom: 3px;
          color: #87efff;
          font-size: 0.63rem;
          font-weight: 800;
          letter-spacing: 0.08em;
          text-transform: uppercase;
        }

        .mine .meta {
          color: #d5faff;
        }

        form.composer {
          display: flex;
          flex: 0 0 auto;
          gap: 8px;
          margin-top: 10px;
        }

        .composer input {
          flex: 1;
        }

        .composer button {
          min-width: 70px;
        }

        @keyframes message-in {
          from {
            opacity: 0;
            transform: translateY(7px);
          }

          to {
            opacity: 1;
            transform: translateY(0);
          }
        }

        @media (max-width: 850px) {
          :host {
            min-height: 700px;
            overflow-y: auto;
          }

          .app {
            width: min(100% - 22px, 680px);
            height: auto;
            min-height: 100%;
            padding: 16px 0;
          }

          .grid {
            grid-template-columns: 1fr;
          }

          aside {
            display: grid;
            grid-template-columns: repeat(3, 1fr);
          }

          aside .card {
            min-height: auto;
          }

          main.card {
            min-height: 430px;
          }

          #messages {
            min-height: 330px;
          }
        }

        @media (max-width: 620px) {
          aside {
            display: flex;
          }

          .app {
            width: calc(100% - 18px);
          }

          h1 {
            font-size: 3.25rem;
          }

          .card {
            padding: 14px;
          }

          main.card {
            min-height: 430px;
          }

          .message {
            max-width: 88%;
          }
        }
      </style>

      <div class="app">
        <header>
          <div class="muted">
            Encrypted, signed messaging over MoQ
          </div>
          <h1>Secure Chat</h1>
        </header>

        <div class="grid">
          <aside>
            <section class="card">
              <h2>Your identity</h2>

              <label>
                Display name
                <input
                  id="display-name"
                  autocomplete="off"
                  placeholder="How should others see you?"
                >
              </label>

              <label>
                Encryption key
                <input id="encryption-key" readonly>
              </label>

              <label>
                Signing private key
                <textarea
                  id="signing-private-key"
                  readonly
                ></textarea>
              </label>

              <label>
                Signing public key
                <textarea
                  id="signing-public-key"
                  readonly
                ></textarea>
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
                <input
                  id="publish-relay"
                  placeholder="https://relay.example/anon"
                >
              </label>

              <label>
                Broadcast name
                <input
                  id="publish-name"
                  placeholder="room/alice"
                >
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
                <input
                  id="subscribe-relay"
                  placeholder="https://relay.example/anon"
                >
              </label>

              <label>
                Broadcast name
                <input
                  id="subscribe-name"
                  placeholder="room/alice"
                >
              </label>

              <label>
                Encryption key
                <input
                  id="remote-encryption-key"
                  placeholder="Paste encryption key"
                >
              </label>

              <label>
                Broadcaster signing public key
                <textarea
                  id="remote-public-key"
                  placeholder="Paste public key"
                ></textarea>
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
              <input
                id="message"
                placeholder="Write an encrypted message..."
                autocomplete="off"
              >
              <button type="submit">Send</button>
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

