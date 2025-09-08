use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub struct RconClient {
    stream: TcpStream,
    request_id: i32,
    buffer: Vec<u8>,
}

impl RconClient {
    pub async fn connect(addr: &str, password: &str) -> Result<RconClient> {
        let stream = TcpStream::connect(addr).await?;
        let mut client = RconClient {
            stream,
            request_id: 0,
            buffer: Vec::with_capacity(4096),
        };

        client.login(password).await?;

        Ok(client)
    }

    /// Send a command to the server
    pub async fn cmd(&mut self, command: &str) -> Result<String> {
        self.build_packet(2, command);
        self.send_packet().await?;

        let (_, id, _, payload) = self.read_packet().await?;
        if id != self.request_id {
            anyhow::bail!(
                "Mismatched response id (expected {}, got {id})",
                self.request_id,
            );
        }
        Ok(payload)
    }

    fn build_packet(&mut self, kind: i32, payload: &str) {
        self.request_id += 1;
        let payload_bytes = payload.as_bytes();
        let length = (4 + 4 + payload_bytes.len() + 2) as i32;

        self.buffer.clear();

        self.buffer.extend_from_slice(&length.to_le_bytes());
        self.buffer
            .extend_from_slice(&self.request_id.to_le_bytes());
        self.buffer.extend_from_slice(&kind.to_le_bytes());
        self.buffer.extend_from_slice(payload_bytes);
        self.buffer.extend_from_slice(&[0, 0]);
    }

    async fn read_packet(&mut self) -> Result<(i32, i32, i32, String)> {
        let mut full_payload = Vec::new();
        let mut last_id;
        let mut last_kind;

        loop {
            if self.buffer.len() < 4096 {
                self.buffer.resize(4096, 0);
            }

            let n = self.stream.read(&mut self.buffer).await?;
            if n == 0 {
                anyhow::bail!("Connection closed unexpectedly");
            }

            println!("Read: {n}");

            let length = i32::from_le_bytes(self.buffer[0..4].try_into()?);
            let id = i32::from_le_bytes(self.buffer[4..8].try_into()?);
            let kind = i32::from_le_bytes(self.buffer[8..12].try_into()?);

            last_id = id;
            last_kind = kind;

            let payload_len = (length - 4 - 4 - 2) as usize;
            full_payload.extend_from_slice(&self.buffer[12..12 + payload_len]);

            if payload_len < 4096 - 12 {
                break;
            }
        }

        Ok((
            full_payload.len() as i32,
            last_id,
            last_kind,
            String::from_utf8_lossy(&full_payload).to_string(),
        ))
    }

    async fn send_packet(&mut self) -> Result<()> {
        self.stream.write_all(&self.buffer).await?;
        Ok(())
    }

    async fn login(&mut self, password: &str) -> Result<()> {
        self.build_packet(3, password);
        self.send_packet().await?;

        let (_, id, _, _) = self.read_packet().await?;
        if id == -1 {
            anyhow::bail!("RCON auth failed");
        }

        Ok(())
    }
}
