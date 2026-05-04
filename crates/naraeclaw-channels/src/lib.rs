//! Channel implementations for NaraeClaw.
//!
//! Supported surfaces:
//! - `cli`     — interactive terminal session
//! - `webhook` — thin HTTP receiver for external integrations (Slack, Telegram, etc.)

pub mod cli;
pub mod orchestrator;
pub mod util;

#[cfg(feature = "channel-webhook")]
pub mod webhook;
