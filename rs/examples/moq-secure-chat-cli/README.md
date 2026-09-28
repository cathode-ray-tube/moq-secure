# moq-secure-chat-cli

This command-line application demonstrates end-to-end encryption and Ed25519 signing for text chat messages over MoQ. It uses [`moq-secure-chat`](https://github.com/cathode-ray-tube/moq-secure/tree/main/rs/examples/moq-secure-chat), a chat wrapper around [`moq-secure`](https://github.com/cathode-ray-tube/moq-secure/tree/main/rs/moq-secure).

A publisher creates a broadcast and sends encrypted, signed messages. A subscriber consumes the broadcast, verifies the signatures, decrypts the messages, and prints them to the terminal.

## Prerequisites

Install Rust using `rustup` if it is not already installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Install `moq-relay`, the relay server, from the MoQ repository. Full instructions are available [here](https://github.com/moq-dev/moq/tree/main/rs/moq-relay).

## Start a Local Relay

Download the example localhost configuration:

```bash
wget https://raw.githubusercontent.com/moq-dev/moq/refs/heads/main/demo/relay/localhost.toml
```

Start the relay:

```bash
moq-relay localhost.toml
```

The example configuration listens at:

```text
https://localhost:4443/chat
```

## Install the CLI

In a second terminal, install the command-line application:

```bash
cargo install moq-secure-chat-cli
```

You can also run it directly from a local checkout with Cargo:

```bash
cargo run -- --help
```

## Publish Messages

Start a publisher using the local relay:

```bash
moq-secure-chat-cli \
  --relay https://localhost:4443/chat \
  --tls-disable-verify \
  publish
```

The publisher automatically generates:

- A random broadcast name
- A random key ID
- A random 256-bit AEAD key
- A random 32-byte Ed25519 signing seed

The default encryption algorithm is ChaCha20-Poly1305.

After starting, the publisher prints a command similar to this:

```text
=== Copy/paste subscriber command ===
moq-secure-chat-cli --relay 'https://localhost:4443/chat' --broadcast '...' --key-id 42 --aead-key '...' --signing-public-key '...' --encryption chacha20-poly1305 --tls-disable-verify subscribe
=== Publisher running ===
Broadcast: ...
Track: chat
Encryption: chacha20-poly1305
Type lines on stdin; press Ctrl+C to quit.
```

Copy and run the displayed subscriber command in a third terminal.

Type messages into the publisher terminal. Verified and decrypted messages will appear in the subscriber terminal with a timestamp:

```text
[14:32:10] Hello from the publisher
```

## Subscribe to a Broadcast

A subscriber requires the publisher's broadcast name, key ID, AEAD key, and Ed25519 public verification key:

```bash
moq-secure-chat-cli \
  --relay https://localhost:4443/chat \
  --tls-disable-verify \
  --broadcast BROADCAST_NAME \
  --key-id KEY_ID \
  --aead-key AEAD_KEY \
  --signing-public-key SIGNING_PUBLIC_KEY \
  subscribe
```

The subscriber waits for the specified broadcast to become available. If the broadcast goes offline, it continues waiting for it to return.

The `--signing-public-key` option is required in subscribe mode. It must contain the publisher's Ed25519 public verification key in hexadecimal or Base64 format.

The encryption algorithm does not need to be specified by the subscriber. It is selected from each received message's wire header.

## Use a Remote Relay

Replace the relay URL with the URL, port, and path of your relay:

```bash
moq-secure-chat-cli \
  --relay https://example.com:4443/chat \
  publish
```

The publisher's generated subscriber command contains the appropriate relay, broadcast, and key parameters.

For a relay using a locally generated or otherwise untrusted TLS certificate, disable certificate verification:

```bash
moq-secure-chat-cli \
  --relay https://example.com:4443/chat \
  --tls-disable-verify \
  publish
```

Avoid `--tls-disable-verify` in production unless certificate verification is handled by another trusted mechanism.

## Encryption Algorithms

The publisher supports the following AEAD algorithms:

| CLI value | Aliases | Description |
|---|---|---|
| `chacha20-poly1305` | `chacha`, `chacha20` | Default encryption algorithm |
| `aes-256-gcm` | `aes`, `aes256-gcm`, `aes256gcm` | AES-256-GCM encryption |

Select AES-256-GCM when publishing with:

```bash
moq-secure-chat-cli \
  --relay https://localhost:4443/chat \
  --tls-disable-verify \
  --encryption aes-256-gcm \
  publish
```

The generated subscriber command includes the selected encryption value for reference, but subscribers determine the algorithm from each received frame.

## Persistent Keys and Broadcast Names

By default, the application generates new values every time it runs. You can provide your own values when publishing:

```bash
moq-secure-chat-cli \
  --relay https://localhost:4443/chat \
  --tls-disable-verify \
  --broadcast my-chat \
  --key-id 7 \
  --aead-key 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --ed25519-signing-seed 00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff \
  publish
```

The Ed25519 signing seed must decode to exactly 32 raw bytes. Hexadecimal and Base64 encodings are supported. It must not be a PEM, PKCS#8, OpenSSH, expanded, or 64-byte private-key representation.

The AEAD key must decode to a valid 32-byte key. Keep the AEAD key and signing seed secret.

## Command-Line Options

Run the following command to display the current options:

```bash
moq-secure-chat-cli --help
```

Important options include:

| Option | Description |
|---|---|
| `--relay <URL>` | MoQ relay endpoint. Required. |
| `--broadcast <NAME>` | Broadcast name. Generated in publish mode if omitted; required in subscribe mode. |
| `--tls-disable-verify` | Disables TLS certificate verification. |
| `--encryption <ALGORITHM>` | Encryption algorithm used by the publisher. |
| `--key-id <ID>` | Optional key ID. Generated if omitted. |
| `--aead-key <KEY>` | Optional AEAD key. Generated if omitted. |
| `--ed25519-signing-seed <SEED>` | Publisher's 32-byte Ed25519 signing seed in hex or Base64. |
| `--signing-public-key <KEY>` | Publisher's Ed25519 public verification key. Required for subscribers. |

The signing-seed option also accepts the alias:

```text
--signing-private-seed
```

## Troubleshooting

Display command-line help:

```bash
moq-secure-chat-cli --help
```

Display help for a specific role:

```bash
moq-secure-chat-cli \
  --relay https://localhost:4443/chat \
  publish --help
```

```bash
moq-secure-chat-cli \
  --relay https://localhost:4443/chat \
  subscribe --help
```

Common issues include:

- Omitting `--tls-disable-verify` when using the example localhost certificate.
- Using `subscriber` instead of the correct subcommand, `subscribe`.
- Omitting `--broadcast` in subscribe mode.
- Providing an AEAD key that is not exactly 32 bytes after decoding.
- Providing an Ed25519 signing seed that is not exactly 32 bytes after decoding.
- Using a signing public key that does not correspond to the publisher's signing seed.
- Using mismatched key IDs or AEAD keys between the publisher and subscriber.

## Security Notes

This is a demonstration application with usability prioritized over production hardening.

For production use, carefully design:

- Key generation, storage, rotation, and distribution
- Access control for broadcasts
- Authentication of participants
- Protection of command-line arguments and shell history
- TLS certificate management
- Secure handling of signing seeds and AEAD keys

The generated subscriber command contains secret key material. Treat it as sensitive and avoid sharing it through insecure channels.
