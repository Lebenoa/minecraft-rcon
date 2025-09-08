mod client;
pub use client::*;

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[tokio::test]
    async fn test_build_and_read_packet() -> Result<()> {
        let mut client = RconClient::connect("127.0.0.1:25575", "fQV0sslyxh").await?;

        let response = client.cmd("give @a cooked_porkchop").await?;
        println!("Rcon response: {response}");

        Ok(())
    }

    #[tokio::test]
    async fn test_sword_with_custom_name() -> Result<()> {
        let mut client = RconClient::connect("127.0.0.1:25575", "fQV0sslyxh").await?;

        let response = client.cmd("give @a minecraft:netherite_sword[minecraft:custom_name={italic:false,text:\"Magic Sword\",color:\"light_purple\"}]").await?;
        println!("Rcon response: {response}");

        Ok(())
    }

    // No vanilla command that returns long enough response
    #[allow(dead_code)]
    async fn test_long_response() -> Result<()> {
        let mut client = RconClient::connect("127.0.0.1:25575", "fQV0sslyxh").await?;

        let response = client.cmd("recipe give @a *").await?;
        println!("Rcon response: {response}");

        Ok(())
    }
}
