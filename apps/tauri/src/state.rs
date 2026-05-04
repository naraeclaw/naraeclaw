//! Shared application state for Tauri.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};

/// Agent status as reported by the gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Working,
    Error,
}

/// Shared application state behind an `Arc<RwLock<_>>`.
#[derive(Debug, Clone)]
pub struct AppState {
    pub gateway_url: String,
    pub token: Option<String>,
    pub connected: bool,
    pub agent_status: AgentStatus,
    /// One-shot nonces for computer-use action approvals.
    /// Uses its own inner `Arc<Mutex<>>` so it can be mutated under a
    /// read lock on `AppState`.
    pub approvals: ApprovalStore,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            gateway_url: "http://127.0.0.1:42617".to_string(),
            token: None,
            connected: false,
            agent_status: AgentStatus::Idle,
            approvals: ApprovalStore::default(),
        }
    }
}

/// Thread-safe wrapper around `AppState`.
pub type SharedState = Arc<RwLock<AppState>>;

/// One-shot approval nonces for destructive computer-use actions.
///
/// Each nonce is valid for `APPROVAL_TTL` and consumed on first use.
/// The renderer must call `request_computer_use_approval` (which shows a
/// native confirmation dialog) to obtain a nonce before calling any
/// computer-use action. Storing the nonce in Rust state means a renderer-
/// side XSS payload must chain two separate calls (request + act) within
/// the TTL window, and the request call itself triggers a visible OS dialog
/// that the real user can dismiss.
pub const APPROVAL_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Default, Clone)]
pub struct ApprovalStore {
    inner: Arc<Mutex<HashMap<String, Instant>>>,
}

impl ApprovalStore {
    /// Insert a new nonce with the current timestamp.
    pub async fn insert(&self, nonce: String) {
        let mut map = self.inner.lock().await;
        // Evict expired entries to avoid unbounded growth.
        map.retain(|_, issued| issued.elapsed() < APPROVAL_TTL);
        map.insert(nonce, Instant::now());
    }

    /// Consume a nonce — returns `true` if valid and removes it.
    pub async fn consume(&self, nonce: &str) -> bool {
        let mut map = self.inner.lock().await;
        match map.get(nonce) {
            Some(issued) if issued.elapsed() < APPROVAL_TTL => {
                map.remove(nonce);
                true
            }
            _ => {
                map.remove(nonce); // clean up expired
                false
            }
        }
    }
}

/// Create the default shared state.
pub fn shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let state = AppState::default();
        assert_eq!(state.gateway_url, "http://127.0.0.1:42617");
        assert!(state.token.is_none());
        assert!(!state.connected);
        assert_eq!(state.agent_status, AgentStatus::Idle);
    }

    #[test]
    fn shared_state_is_cloneable() {
        let s1 = shared_state();
        let s2 = s1.clone();
        // Both references point to the same allocation.
        assert!(Arc::ptr_eq(&s1, &s2));
    }

    #[tokio::test]
    async fn shared_state_concurrent_read_write() {
        let state = shared_state();

        // Write from one handle.
        {
            let mut s = state.write().await;
            s.connected = true;
            s.agent_status = AgentStatus::Working;
            s.token = Some("zc_test".to_string());
        }

        // Read from cloned handle.
        let state2 = state.clone();
        let s = state2.read().await;
        assert!(s.connected);
        assert_eq!(s.agent_status, AgentStatus::Working);
        assert_eq!(s.token.as_deref(), Some("zc_test"));
    }

    #[test]
    fn agent_status_serialization() {
        assert_eq!(
            serde_json::to_string(&AgentStatus::Idle).unwrap(),
            "\"idle\""
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::Working).unwrap(),
            "\"working\""
        );
        assert_eq!(
            serde_json::to_string(&AgentStatus::Error).unwrap(),
            "\"error\""
        );
    }
}
