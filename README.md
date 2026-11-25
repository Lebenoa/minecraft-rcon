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

`connect` logs in and fails with `RCON authentication failed` when the password is wrong. `cmd` returns the command's output with the trailing newline trimmed. Neither has a built-in deadline; call `connect_with_timeout` / `cmd_with_timeout` when you need one.

## API reference

| Item | Signature | Behavior |
|---|---|---|
| `RconClient::connect` | `async fn connect(addr: &str, password: &str) -> anyhow::Result<RconClient>` | Opens `addr` (`host:port`) and authenticates; waits as long as the server takes |
| `RconClient::connect_with_timeout` | `async fn connect_with_timeout(addr: &str, password: &str, timeout: Duration) -> anyhow::Result<RconClient>` | Same, failing once `timeout` elapses while connecting or authenticating |
| `RconClient::cmd` | `async fn cmd(&mut self, command: &str) -> anyhow::Result<String>` | Sends one command; collects replies until the idle window passes or 64 KiB arrive |
| `RconClient::cmd_with_timeout` | `async fn cmd_with_timeout(&mut self, command: &str, timeout: Duration) -> anyhow::Result<String>` | Same, failing once `timeout` elapses; discard the client after a timeout |
| `RconClient::set_idle_window` | `fn set_idle_window(&mut self, idle_window: Duration)` | Sets the gap between packets that ends a response; 250 ms by default |

All return `anyhow::Error` with context (`connecting to …`, `reading RCON auth response`, `sending RCON packet`). The client owns one connection; commands are serialized by `&mut self`.

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
- One command per `cmd` call. Output is collected until the idle window (250 ms by default, see `set_idle_window`) passes without a new packet, so a slow server can add that delay and a very long response can be cut at 64 KiB.
- No built-in deadline: a server that accepts the connection and never answers blocks `connect`/`cmd` until you cancel the future or use the `_with_timeout` variants.
- No automatic reconnect: a dropped connection surfaces as an error on the next call; reconnect by calling `connect` again.
- The client does not retry, throttle, or check command syntax; the server validates commands and returns its error text as output.

## License

MIT. See [LICENSE](LICENSE).
