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
  #subscriptions = new Map<string, MoqChatSubscription>();

  connectedCallback(): void {
    this.render();
    this.bindEvents();
  }

  private render(): void {
    this.innerHTML = `
      <style>
        :host {
          --electric: #38e8ff;
          --electric-2: #6478ff;
          --text: #f2fcff;
          --muted: #9acde8;

          position: fixed;
          inset: 0;
          display: block;
          width: 100vw;
          height: 100dvh;
          min-height: 0;
          overflow: hidden;
          color: var(--text);
          background:
            radial-gradient(
              ellipse at 15% 0%,
              #0875c9 0%,
              transparent 38%
            ),
            radial-gradient(
              ellipse at 90% 100%,
              #123eb1 0%,
              transparent 42%
            ),
            linear-gradient(
              135deg,
              #021f52 0%,
              #032c6a 48%,
              #064c92 100%
            );
          font-family: Inter, system-ui, sans-serif;
          color-scheme: dark;
        }

        :host::before {
          content: "";
          position: absolute;
          inset: 0;
          pointer-events: none;
          opacity: .25;
          background-image:
            linear-gradient(
              rgba(117, 235, 255, .16) 1px,
              transparent 1px
            ),
            linear-gradient(
              90deg,
              rgba(117, 235, 255, .16) 1px,
              transparent 1px
            );
          background-size: 36px 36px;
        }

        * {
          box-sizing: border-box;
        }

        .app {
          position: relative;
          z-index: 1;
          display: flex;
          flex-direction: column;
          width: min(1300px, calc(100% - 28px));
          height: 100%;
          min-height: 0;
          margin: auto;
          padding: 14px 0;
        }

        header {
          flex: 0 0 auto;
          margin-bottom: 12px;
        }

        header::after {
          content: "";
          display: block;
          width: 110px;
          height: 3px;
          margin-top: 9px;
          border-radius: 99px;
          background: linear-gradient(
            90deg,
            var(--electric),
            var(--electric-2)
          );
          box-shadow:
            0 0 12px var(--electric),
            0 0 30px var(--electric-2);
        }

        h1,
        h2 {
          font-family: "Space Grotesk", Inter, sans-serif;
        }

        h1 {
          margin: 3px 0 0;
          color: #fff;
          font-size: clamp(2.5rem, 6vw, 5rem);
          line-height: .86;
          letter-spacing: -.09em;
          text-shadow:
            0 0 12px #38e8ff,
            0 0 36px #38e8ff;
        }

        h2 {
          margin: 0 0 10px;
          font-size: .98rem;
        }

        .muted {
          color: #a1f4ff;
          font-size: .65rem;
          font-weight: 800;
          letter-spacing: .16em;
          text-transform: uppercase;
        }

        .grid {
          display: grid;
          grid-template-columns: 355px minmax(0, 1fr);
          gap: 13px;
          flex: 1;
          min-height: 0;
        }

        aside {
          display: flex;
          flex-direction: column;
          gap: 10px;
          min-height: 0;
          overflow-y: auto;
          padding: 2px;
          scrollbar-color: var(--electric) transparent;
        }

        .card {
          position: relative;
          overflow: hidden;
          border: 1px solid rgba(111, 239, 255, .88);
          border-radius: 15px;
          background:
            linear-gradient(
              145deg,
              rgba(4, 69, 133, .96),
              rgba(2, 34, 83, .96)
            );
          box-shadow:
            0 0 0 1px rgba(43, 203, 255, .22),
            0 0 17px rgba(39, 216, 255, .3),
            inset 0 1px rgba(255, 255, 255, .18);
        }

        aside .card {
          flex: 0 0 auto;
          padding: 13px;
        }

        .card::before {
          content: "";
          position: absolute;
          top: 0;
          right: 12%;
          left: 12%;
          height: 2px;
          background: var(--electric);
          box-shadow: 0 0 13px var(--electric);
        }

        main.card {
          display: flex;
          flex-direction: column;
          min-width: 0;
          min-height: 0;
          padding: 11px;
        }

        label {
          display: grid;
          gap: 4px;
          margin-top: 8px;
          color: var(--muted);
          font-size: .62rem;
          font-weight: 800;
          letter-spacing: .04em;
          text-transform: uppercase;
        }

        input,
        textarea {
          width: 100%;
          border: 1px solid rgba(106, 228, 255, .55);
          border-radius: 7px;
          padding: 7px 9px;
          outline: none;
          color: var(--text);
          background: rgba(1, 22, 59, .88);
          font: inherit;
          font-size: .76rem;
        }

        input::placeholder,
        textarea::placeholder {
          color: #78a8c7;
        }

        input:focus,
        textarea:focus {
          border-color: #b8faff;
          box-shadow:
            0 0 0 2px rgba(56, 232, 255, .2),
            0 0 17px rgba(56, 232, 255, .55);
        }

        input[readonly],
        textarea[readonly] {
          color: #8ef1ff;
          background: rgba(0, 48, 94, .9);
        }

        textarea {
          min-height: 48px;
          resize: none;
          line-height: 1.25;
        }

        button {
          border: 1px solid #c1faff;
          border-radius: 7px;
          padding: 8px 11px;
          color: #00172f;
          background: linear-gradient(
            135deg,
            #a5faff,
            #32dbff 48%,
            #7181ff
          );
          box-shadow:
            0 0 13px rgba(56, 232, 255, .65),
            inset 0 1px #fff;
          font: inherit;
          font-size: .7rem;
          font-weight: 800;
          cursor: pointer;
        }

        button:hover {
          filter: brightness(1.18);
          box-shadow:
            0 0 25px rgba(56, 232, 255, .95),
            inset 0 1px #fff;
        }

        .actions {
          display: flex;
          gap: 7px;
          margin-top: 10px;
        }

        #messages {
          flex: 1;
          min-height: 0;
          overflow-y: auto;
          padding: 9px;
          border: 1px solid rgba(111, 239, 255, .65);
          border-radius: 10px;
          background:
            radial-gradient(
              ellipse at 50% 0%,
              rgba(29, 174, 255, .3),
              transparent 55%
            ),
            rgba(0, 20, 56, .75);
          scrollbar-color: var(--electric) transparent;
        }

        #messages:empty::before {
          content: "Messages will appear here";
          display: grid;
          height: 100%;
          place-items: center;
          color: #9bd4e8;
          font-family: "Space Grotesk", sans-serif;
          font-size: .82rem;
        }

        .message {
          max-width: 76%;
          margin: 8px 0;
          padding: 9px 12px;
          border: 1px solid rgba(101, 232, 255, .55);
          border-radius: 5px 13px 13px 13px;
          background: rgba(5, 104, 164, .85);
          box-shadow: 0 0 13px rgba(45, 210, 255, .2);
          font-size: .82rem;
          line-height: 1.35;
        }

        .mine {
          margin-left: auto;
          border-color: #a0aaff;
          border-radius: 13px 5px 13px 13px;
          background: linear-gradient(
            135deg,
            rgba(5, 153, 190, .9),
            rgba(61, 70, 178, .92)
          );
          box-shadow: 0 0 15px rgba(105, 125, 255, .4);
        }

        .meta {
          margin-bottom: 3px;
          color: #9df5ff;
          font-size: .6rem;
          font-weight: 800;
          letter-spacing: .08em;
          text-transform: uppercase;
        }

        .mine .meta {
          color: #e0fdff;
        }

        form.composer {
          display: flex;
          flex: 0 0 auto;
          gap: 7px;
          margin-top: 9px;
        }

        .composer input {
          flex: 1;
        }

        .composer button {
          min-width: 68px;
        }

        @media (max-width: 900px) {
          :host {
            overflow-y: auto;
          }

          .app {
            height: auto;
            min-height: 100%;
            padding: 13px 0;
          }

          .grid {
            grid-template-columns: 1fr;
          }

          aside {
            display: grid;
            grid-template-columns: repeat(3, minmax(0, 1fr));
            overflow: visible;
          }

          aside .card {
            min-width: 0;
          }

          main.card {
            height: 55dvh;
            min-height: 360px;
          }
        }

        @media (max-width: 620px) {
          aside {
            display: flex;
          }

          main.card {
            height: 55dvh;
          }

          h1 {
            font-size: 3rem;
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
    const element = this.querySelector(`#${id}`);

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
