# moq-secure-chat-cli

This demonstrates [moq-secure](https://github.com/cathode-ray-tube/moq-secure) end-to-end encryption and signing of text chat messages in the terminal. It implements [moq-secure-chat](https://github.com/cathode-ray-tube/moq-secure/tree/main/rs/moq-secure-chat), a wrapper providing chat functionality around the encryption/signing library, [moq-secure](https://github.com/cathode-ray-tube/moq-secure/tree/main/rs/moq-secure).

### Prerequisites

Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Install **moq-relay** (the server) from the moq repo, full instructions [here](https://github.com/moq-dev/moq/tree/main/rs/moq-relay).

### Run

Run moq-relay with this config file, [localhost.toml](https://github.com/moq-dev/moq/blob/main/demo/relay/localhost.toml):

```bash
wget https://raw.githubusercontent.com/moq-dev/moq/refs/heads/main/demo/relay/localhost.toml
moq-relay localhost.toml
```

In a **2nd terminal**, clone this repo:

```bash
git clone https://github.com/cathode-ray-tube/moq-secure.git
```

Build:

```bash
cd moq-secure
cargo build -p moq-secure-chat-cli
```

Run:

```bash
cd target/debug
./moq-secure-chat-cli --relay https://localhost:4443/chat --broadcast chat --tls-disable-verify publish
```
In a **3rd terminal**, paste the displayed command to run the binary in subscriber mode and receive sent messages.

### Troubleshooting

Run binary with `-h` or `--help` flag to list available args and usage:

```bash
./moq-secure-chat-cli --help
```
