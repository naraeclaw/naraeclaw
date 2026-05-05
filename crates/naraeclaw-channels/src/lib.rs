//! Channel implementations for NaraeClaw.
//!
//! Supported surfaces:
//! - `cli`   — interactive terminal session
//! - `slack` — Slack Socket Mode bot (no public URL required)

pub mod cli;
pub mod orchestrator;
pub mod util;

#[cfg(feature = "channel-slack")]
pub mod slack;

#[cfg(feature = "channel-slack")]
pub use crate::slack::SlackChannel;
