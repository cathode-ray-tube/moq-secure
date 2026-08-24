## moq-secure-chat-cli

This demonstrates moq-secure end-to-end encryption and signing of text chat messages in the terminal.

## Run:

First, install and run a moq-relay from the moq repo, instructions [here](https://github.com/moq-dev/moq/tree/main/rs/moq-relay).

Clone:

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
In a second terminal, paste the displayed command to run the binary in subscriber mode and receive sent messages.

Run binary with `-h` or `--help` flag to list available args.
