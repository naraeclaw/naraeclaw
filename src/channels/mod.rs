pub use naraeclaw_channels::orchestrator::*;
pub mod session_backend {
    pub use naraeclaw_infra::session_backend::*;
}
pub mod session_sqlite {
    pub use naraeclaw_infra::session_sqlite::*;
}

use crate::config::Config;
use anyhow::Result;

pub async fn handle_command(command: crate::ChannelCommands, config: &Config) -> Result<()> {
    match command {
        crate::ChannelCommands::Start => {
            anyhow::bail!("Start must be handled in main.rs (requires async runtime)")
        }
        crate::ChannelCommands::Doctor => {
            anyhow::bail!("Doctor must be handled in main.rs (requires async runtime)")
        }
        crate::ChannelCommands::List => {
            println!("Channels:");
            println!("  ✅ CLI (always available)");
            #[cfg(feature = "channel-slack")]
            if config
                .channels_config
                .slack
                .as_ref()
                .is_some_and(|s| s.enabled)
            {
                println!("  ✅ Slack (Socket Mode)");
            }
            println!("\nTo start channels: naraeclaw channel start");
            println!("To check health:    naraeclaw channel doctor");
            println!("To configure:      naraeclaw onboard");
            Ok(())
        }
        crate::ChannelCommands::Add {
            channel_type,
            config: _,
        } => {
            anyhow::bail!(
                "Channel type '{channel_type}' — use `naraeclaw onboard` to configure channels"
            );
        }
        crate::ChannelCommands::Remove { name } => {
            anyhow::bail!("Remove channel '{name}' — edit ~/.naraeclaw/config.toml directly");
        }
        crate::ChannelCommands::Send {
            message,
            channel_id,
            recipient,
        } => send_channel_message(config, &channel_id, &recipient, &message).await,
    }
}
