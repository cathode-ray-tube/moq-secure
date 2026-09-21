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
        @import url("https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=Space+Grotesk:wght@400;500;600;700&display=swap");

        :host {
          --bg: #050914;
          --panel: rgba(10, 22, 45, 0.78);
          --line: rgba(92, 180, 255, 0.2);
          --text: #eaf6ff;
          --muted: #829abb;
          --blue: #38bdf8;
          --cyan: #22d3ee;
          --purple: #818cf8;

          display: block;
          min-height: 100vh;
          overflow-x: hidden;
          color: var(--text);
          background:
            radial-gradient(
              circle at 8% 8%,
              rgba(24, 105, 189, 0.32),
              transparent 30%
            ),
            radial-gradient(
              circle at 92% 20%,
              rgba(73, 68, 191, 0.2),
              transparent 28%
            ),
            linear-gradient(
              135deg,
              #071225 0%,
              #030711 48%,
              #08162b 100%
            );
          font-family: "Inter", system-ui, sans-serif;
        }

        :host::before {
          content: "";
          position: fixed;
          inset: 0;
          pointer-events: none;
          opacity: 0.18;
          background-image:
            linear-gradient(
              rgba(73, 157, 255, 0.08) 1px,
              transparent 1px
            ),
            linear-gradient(
              90deg,
              rgba(73, 157, 255, 0.08) 1px,
              transparent 1px
            );
          background-size: 42px 42px;
          mask-image: linear-gradient(
            to bottom,
            black,
            transparent 85%
          );
        }

        * {
          box-sizing: border-box;
        }

        .app {
          position: relative;
          z-index: 1;
          width: min(1240px, calc(100% - 36px));
          margin: auto;
          padding: 54px 0 70px;
        }

        header {
          position: relative;
          margin-bottom: 32px;
          padding: 4px 0 12px;
        }

        header::after {
          content: "";
          display: block;
          width: 110px;
          height: 3px;
          margin-top: 20px;
          border-radius: 99px;
          background: linear-gradient(
            90deg,
            var(--cyan),
            var(--purple)
          );
          box-shadow:
            0 0 12px rgba(34, 211, 238, 0.8),
            0 0 32px rgba(129, 140, 248, 0.5);
        }

        h1,
        h2 {
          font-family: "Space Grotesk", sans-serif;
        }

        h1 {
          margin: 7px 0 0;
          color: #f2fbff;
          font-size: clamp(3rem, 8vw, 6.8rem);
          font-weight: 700;
          letter-spacing: -0.095em;
          line-height: 0.92;
          text-shadow:
            0 0 12px rgba(56, 189, 248, 0.55),
            0 0 38px rgba(56, 189, 248, 0.18);
        }

        h2 {
          margin: 0 0 20px;
          color: #f4fbff;
          font-size: 1.15rem;
          font-weight: 600;
          letter-spacing: -0.035em;
        }

        .muted {
          color: var(--cyan);
          font-size: 0.76rem;
          font-weight: 700;
          letter-spacing: 0.16em;
          text-transform: uppercase;
        }

        .grid {
          display: grid;
          grid-template-columns: 370px minmax(0, 1fr);
          gap: 22px;
          align-items: start;
        }

        .card {
          position: relative;
          padding: 24px;
          overflow: hidden;
          border: 1px solid var(--line);
          border-radius: 22px;
          background:
            linear-gradient(
              145deg,
              rgba(18, 47, 87, 0.72),
              rgba(6, 16, 34, 0.84)
            );
          box-shadow:
            0 24px 70px rgba(0, 0, 0, 0.34),
            inset 0 1px rgba(255, 255, 255, 0.07);
          backdrop-filter: blur(22px);
        }

        .card::before {
          content: "";
          position: absolute;
          top: 0;
          right: 18%;
          left: 18%;
          height: 1px;
          background: linear-gradient(
            90deg,
            transparent,
            rgba(56, 189, 248, 0.8),
            transparent
          );
        }

        .card + .card {
          margin-top: 18px;
        }

        label {
          display: grid;
          gap: 8px;
          margin-top: 17px;
          color: #9db4d3;
          font-size: 0.76rem;
          font-weight: 600;
          letter-spacing: 0.04em;
          text-transform: uppercase;
        }

        input,
        textarea {
          width: 100%;
          border: 1px solid rgba(113, 174, 225, 0.2);
          border-radius: 11px;
          padding: 12px 13px;
          outline: none;
          color: var(--text);
          background: rgba(2, 9, 22, 0.7);
          font: inherit;
          font-size: 0.88rem;
          transition:
            border-color 160ms ease,
            box-shadow 160ms ease,
            background 160ms ease;
        }

        input::placeholder,
        textarea::placeholder {
          color: #59718f;
        }

        input:focus,
        textarea:focus {
          border-color: var(--cyan);
          background: rgba(4, 17, 36, 0.9);
          box-shadow:
            0 0 0 3px rgba(34, 211, 238, 0.1),
            0 0 20px rgba(34, 211, 238, 0.12);
        }

        input[readonly],
        textarea[readonly] {
          color: #73d8ff;
          border-color: rgba(56, 189, 248, 0.16);
          background: rgba(7, 28, 51, 0.68);
        }

        textarea {
          min-height: 82px;
          resize: vertical;
          line-height: 1.5;
        }

        button {
          border: 1px solid rgba(125, 240, 255, 0.4);
          border-radius: 11px;
          padding: 12px 16px;
          color: #03101d;
          background: linear-gradient(
            135deg,
            #67e8f9 0%,
            #38bdf8 50%,
            #818cf8 100%
          );
          box-shadow:
            0 0 16px rgba(56, 189, 248, 0.2),
            inset 0 1px rgba(255, 255, 255, 0.55);
          font: inherit;
          font-size: 0.82rem;
          font-weight: 800;
          letter-spacing: 0.015em;
          cursor: pointer;
          transition:
            transform 160ms ease,
            filter 160ms ease,
            box-shadow 160ms ease;
        }

        button:hover {
          filter: brightness(1.14);
          transform: translateY(-2px);
          box-shadow:
            0 0 24px rgba(56, 189, 248, 0.45),
            inset 0 1px rgba(255, 255, 255, 0.65);
        }

        button:active {
          transform: translateY(0);
        }

        button.secondary {
          color: #d6f5ff;
          border-color: rgba(130, 180, 230, 0.22);
          background: rgba(255, 255, 255, 0.07);
          box-shadow: none;
        }

        .actions {
          display: flex;
          gap: 10px;
          flex-wrap: wrap;
          margin-top: 20px;
        }

        main.card {
          min-width: 0;
          padding: 18px;
        }

        #messages {
          height: 570px;
          overflow: auto;
          padding: 14px;
          border: 1px solid rgba(100, 167, 220, 0.12);
          border-radius: 16px;
          background:
            radial-gradient(
              circle at 50% 0%,
              rgba(24, 93, 158, 0.13),
              transparent 45%
            ),
            rgba(1, 8, 19, 0.46);
          scrollbar-color: #24648c transparent;
          scrollbar-width: thin;
        }

        #messages:empty::before {
          content: "Messages will appear here";
          display: grid;
          height: 100%;
          place-items: center;
          color: #486682;
          font-family: "Space Grotesk", sans-serif;
          font-size: 0.9rem;
          letter-spacing: 0.05em;
        }

        .message {
          max-width: 76%;
          margin: 12px 0;
          padding: 13px 16px;
          border: 1px solid rgba(109, 170, 220, 0.14);
          border-radius: 6px 18px 18px 18px;
          color: #dff5ff;
          background: rgba(21, 48, 82, 0.68);
          box-shadow: 0 8px 24px rgba(0, 0, 0, 0.14);
          line-height: 1.5;
          animation: message-in 220ms ease-out;
        }

        .mine {
          margin-left: auto;
          border-color: rgba(82, 226, 255, 0.28);
          border-radius: 18px 6px 18px 18px;
          background: linear-gradient(
            135deg,
            rgba(20, 117, 164, 0.8),
            rgba(73, 70, 164, 0.8)
          );
          box-shadow:
            0 8px 28px rgba(25, 124, 198, 0.2),
            inset 0 1px rgba(255, 255, 255, 0.12);
        }

        .meta {
          margin-bottom: 5px;
          color: #78ddf7;
          font-size: 0.7rem;
          font-weight: 800;
          letter-spacing: 0.08em;
          text-transform: uppercase;
        }

        .mine .meta {
          color: #b9f7ff;
        }

        form.composer {
          display: flex;
          gap: 10px;
          margin-top: 16px;
        }

        .composer input {
          flex: 1;
        }

        .composer button {
          min-width: 82px;
        }

        @keyframes message-in {
          from {
            opacity: 0;
            transform: translateY(8px);
          }

          to {
            opacity: 1;
            transform: translateY(0);
          }
        }

        @media (max-width: 820px) {
          .app {
            width: min(100% - 24px, 680px);
            padding-top: 32px;
          }

          .grid {
            grid-template-columns: 1fr;
          }

          h1 {
            font-size: clamp(3rem, 18vw, 5rem);
          }

          #messages {
            height: 460px;
          }
        }

        @media (max-width: 520px) {
          .card {
            padding: 18px;
            border-radius: 18px;
          }

          form.composer {
            flex-direction: column;
          }

          .composer button {
            width: 100%;
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

