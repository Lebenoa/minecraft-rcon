# minecraft-rcon

Runs console commands on a Minecraft server over RCON from async Rust.

![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

```bash
cargo add minecraft-rcon --git https://github.com/Lebenoa/minecraft-rcon
cargo add tokio --features macros,rt-multi-thread
```

Pin a rev for reproducible builds:

```bash
cargo add minecraft-rcon --git https://github.com/Lebenoa/minecraft-rcon --rev f19a2090f246438447adbfd35d343ea5c76fdf24
```

## Quick start

```rust
use minecraft_rcon::RconClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rcon = RconClient::connect("127.0.0.1:25575", "password").await?;
    let output = rcon.cmd("list").await?;
    println!("{output}");
    Ok(())
}
```

```text
There are 2 of a max of 20 players online: Steve, Alex
```

`connect` logs in and fails with `RCON authentication failed` when the password is wrong. `cmd` returns the command's output with the trailing newline trimmed.

## API reference

| Item | Signature | Behavior |
|---|---|---|
| `RconClient::connect` | `async fn connect(addr: &str, password: &str) -> anyhow::Result<RconClient>` | Opens `addr` (`host:port`), authenticates; 2 s timeout for the TCP connect and for the auth reply |
| `RconClient::cmd` | `async fn cmd(&mut self, command: &str) -> anyhow::Result<String>` | Sends one command; collects replies until 250 ms of silence or 64 KiB of output |

Both return `anyhow::Error` with context (`connecting to …`, `reading RCON auth response`, `sending RCON packet`). The client owns one connection; commands are serialized by `&mut self`.

## Examples

Broadcast to every player:

```rust
let _ = rcon.cmd("say Server restart in 5 minutes").await?;
```

Give an item:

```rust
rcon.cmd("give Steve minecraft:bread 16").await?;
```

List players and reuse the same connection:

```rust
for _ in 0..3 {
    println!("{}", rcon.cmd("list").await?);
}
```

## Compatibility

- Rust 1.85+ (edition 2024); tokio 1.x with `net`, `io-util`, `time`, and a runtime.
- Server: `enable-rcon=true`, a `rcon.port`, and a non-empty `rcon.password` in `server.properties`. With an empty password the server logs `No rcon password set in server.properties, rcon disabled!` and ignores the port.
- Protocol: Source RCON. The server answers each packet with the request id; the client rejects mismatched ids and treats `-1` as a failed login.
- Not published to crates.io; install from git as shown above.

## Limitations

- RCON has no encryption: the password and every command travel as plaintext. Keep the port on loopback or a VPN.
- One command per `cmd` call. Output is collected until 250 ms pass without a new packet, so a slow server can add that delay and a very long response can be cut at 64 KiB.
- No automatic reconnect: a dropped connection surfaces as an error on the next call; reconnect by calling `connect` again.
- The client does not retry, throttle, or check command syntax; the server validates commands and returns its error text as output.

## License

MIT. See [LICENSE](LICENSE).
