//! Minimal async RCON client for Minecraft servers.

use std::io;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

const TYPE_RESPONSE: i32 = 0;
const TYPE_COMMAND: i32 = 2;
const TYPE_AUTH: i32 = 3;

const AUTH_TIMEOUT: Duration = Duration::from_secs(2);
const IDLE_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_PACKET: usize = 64 * 1024;

/// A logged-in RCON connection.
pub struct RconClient {
    stream: TcpStream,
    request_id: i32,
}

impl RconClient {
    /// Opens a connection and authenticates it.
    pub async fn connect(addr: &str, password: &str) -> Result<RconClient> {
        let mut stream = timeout(AUTH_TIMEOUT, TcpStream::connect(addr))
            .await
            .map_err(|_| anyhow!("connecting to {addr} timed out"))?
            .with_context(|| format!("connecting to {addr}"))?;

        let mut request_id = 0;
        login(&mut stream, &mut request_id, password).await?;
        Ok(RconClient { stream, request_id })
    }

    /// Runs one command and returns its output.
    pub async fn cmd(&mut self, command: &str) -> Result<String> {
        command_on(&mut self.stream, &mut self.request_id, command).await
    }
}

async fn login<S>(stream: &mut S, request_id: &mut i32, password: &str) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    send_packet(stream, request_id, TYPE_AUTH, password.as_bytes()).await?;
    let expected = *request_id;

    let (id, _, _) = timeout(AUTH_TIMEOUT, read_packet(stream))
        .await
        .map_err(|_| anyhow!("RCON authentication timed out"))?
        .context("reading RCON auth response")?;

    if id == -1 || id != expected {
        bail!("RCON authentication failed");
    }
    Ok(())
}

async fn command_on<S>(stream: &mut S, request_id: &mut i32, command: &str) -> Result<String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    send_packet(stream, request_id, TYPE_COMMAND, command.as_bytes()).await?;
    let expected = *request_id;

    let mut output = Vec::new();
    loop {
        match timeout(IDLE_TIMEOUT, read_packet(stream)).await {
            Ok(Ok((id, kind, body))) => {
                if id != expected {
                    bail!("mismatched response id (expected {expected}, got {id})");
                }
                if kind == TYPE_RESPONSE {
                    output.extend_from_slice(&body);
                }
                if output.len() >= MAX_PACKET {
                    break;
                }
            }
            Ok(Err(error)) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Ok(Err(error)) => return Err(error).context("reading RCON response"),
            Err(_) => break,
        }
    }

    Ok(String::from_utf8_lossy(&output).trim_end().to_owned())
}

async fn send_packet<S>(stream: &mut S, request_id: &mut i32, kind: i32, body: &[u8]) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    *request_id = request_id.wrapping_add(1).max(1);
    write_packet(stream, *request_id, kind, body)
        .await
        .context("sending RCON packet")
}

async fn write_packet<S>(stream: &mut S, id: i32, kind: i32, body: &[u8]) -> io::Result<()>
where
    S: AsyncWrite + Unpin,
{
    let length = body.len() + 10;
    let mut packet = Vec::with_capacity(length + 4);
    packet.extend_from_slice(&(length as i32).to_le_bytes());
    packet.extend_from_slice(&id.to_le_bytes());
    packet.extend_from_slice(&kind.to_le_bytes());
    packet.extend_from_slice(body);
    packet.extend_from_slice(&[0, 0]);
    stream.write_all(&packet).await
}

async fn read_packet<S>(stream: &mut S) -> io::Result<(i32, i32, Vec<u8>)>
where
    S: AsyncRead + Unpin,
{
    let length = stream.read_i32_le().await?;
    if !(10..=MAX_PACKET as i32).contains(&length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid RCON packet length {length}"),
        ));
    }

    let mut payload = vec![0u8; length as usize];
    stream.read_exact(&mut payload).await?;

    let id = i32::from_le_bytes(payload[0..4].try_into().expect("packet id"));
    let kind = i32::from_le_bytes(payload[4..8].try_into().expect("packet type"));
    Ok((id, kind, payload[8..payload.len() - 2].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn authenticates_and_reads_a_response() {
        let (mut client, mut server) = tokio::io::duplex(1024);
        tokio::spawn(async move {
            let (id, kind, body) = read_packet(&mut server).await.unwrap();
            assert_eq!(kind, TYPE_AUTH);
            assert_eq!(body, b"secret");
            write_packet(&mut server, id, TYPE_RESPONSE, b"").await.unwrap();

            let (id, kind, body) = read_packet(&mut server).await.unwrap();
            assert_eq!(kind, TYPE_COMMAND);
            assert_eq!(body, b"list");
            write_packet(&mut server, id, TYPE_RESPONSE, b"2 of 20 players online")
                .await
                .unwrap();
        });

        let mut id = 0;
        login(&mut client, &mut id, "secret").await.unwrap();
        let output = command_on(&mut client, &mut id, "list").await.unwrap();
        assert_eq!(output, "2 of 20 players online");
    }

    #[tokio::test]
    async fn rejects_a_failed_auth() {
        let (mut client, mut server) = tokio::io::duplex(1024);
        tokio::spawn(async move {
            let _ = read_packet(&mut server).await;
            write_packet(&mut server, -1, TYPE_RESPONSE, b"").await.unwrap();
        });

        let mut id = 0;
        let error = login(&mut client, &mut id, "wrong").await.unwrap_err();
        assert!(error.to_string().contains("authentication failed"));
    }

    #[tokio::test]
    async fn reassembles_a_packet_split_across_reads() {
        let (mut client, mut server) = tokio::io::duplex(1024);
        tokio::spawn(async move {
            let (id, _, _) = read_packet(&mut server).await.unwrap();
            write_packet(&mut server, id, TYPE_RESPONSE, b"").await.unwrap();

            let (id, _, _) = read_packet(&mut server).await.unwrap();
            let body = b"2 of 20 players online";
            let mut packet = Vec::new();
            packet.extend_from_slice(&((body.len() + 10) as i32).to_le_bytes());
            packet.extend_from_slice(&id.to_le_bytes());
            packet.extend_from_slice(&TYPE_RESPONSE.to_le_bytes());
            packet.extend_from_slice(body);
            packet.extend_from_slice(&[0, 0]);

            let (head, tail) = packet.split_at(14);
            server.write_all(head).await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            server.write_all(tail).await.unwrap();
        });

        let mut id = 0;
        login(&mut client, &mut id, "secret").await.unwrap();
        let output = command_on(&mut client, &mut id, "list").await.unwrap();
        assert_eq!(output, "2 of 20 players online");
    }
}
