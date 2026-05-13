//! Auto-evolution trigger service (ADR-005 M2, Bit 3).
//!
//! Bundles the three runtime pieces needed to decide whether to spin off a
//! reusable skill from a finished turn:
//! * a [`SkillCreator`] (already config-gated by `skills.skill_creation`),
//! * an optional [`EmbeddingProvider`] for dedup,
//! * the M2 [`SkillAutoEvolutionConfig`] gate.
//!
//! Callers (currently only the agent loop) construct one
//! [`SkillEvolutionService`] up front and call [`Self::try_trigger`] once per
//! finished turn. The service short-circuits cheap rejections (gate off,
//! single-step trace) before doing any I/O, so it is safe to call from the
//! request path; heavy work is moved to a detached task via `tokio::spawn`.

use std::path::Path;
use std::sync::Arc;

use naraeclaw_config::schema::{Config, SkillAutoEvolutionConfig};
use naraeclaw_memory::embeddings::EmbeddingProvider;

use crate::agent::execution_trace::ExecutionTrace;
use crate::agent::value_signal::{self, UserSignal};
use crate::skills::creator::SkillCreator;

/// Outcome of one [`SkillEvolutionService::try_trigger`] decision. Used by
/// callers and tests to assert on what happened without depending on logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerDecision {
    /// `auto_evolution.enabled = false`.
    Disabled,
    /// `score < trigger_threshold` — nothing scheduled.
    BelowThreshold,
    /// `user_signal = Suppress` — explicit user veto.
    Suppressed,
    /// A `tokio::spawn` was issued to attempt skill creation.
    Spawned,
}

/// Trigger-gate service. Cheap to clone — wraps `Arc`s internally.
#[derive(Clone)]
pub struct SkillEvolutionService {
    creator: Arc<SkillCreator>,
    embedding: Option<Arc<dyn EmbeddingProvider>>,
    config: SkillAutoEvolutionConfig,
}

impl SkillEvolutionService {
    pub fn new(
        creator: Arc<SkillCreator>,
        embedding: Option<Arc<dyn EmbeddingProvider>>,
        config: SkillAutoEvolutionConfig,
    ) -> Self {
        Self {
            creator,
            embedding,
            config,
        }
    }

    /// Build a service from the runtime config if (and only if) the
    /// auto-evolution gate is enabled. Returns `None` otherwise so callers
    /// can drop the value straight into `run_tool_call_loop`'s
    /// `skill_evolution: Option<Arc<_>>` slot.
    ///
    /// The embedding provider is intentionally left as `None` for now —
    /// hooking it up to the same provider the memory layer uses requires
    /// exposing more of `naraeclaw-memory`'s internal resolution, and the
    /// `SkillCreator` dedup path treats `None` as "skip the similarity
    /// check", which is the safer initial behaviour.
    pub fn from_config(config: &Config, workspace_dir: &Path) -> Option<Arc<Self>> {
        if !config.skills.auto_evolution.enabled {
            return None;
        }
        let creator = Arc::new(SkillCreator::new(
            workspace_dir.to_path_buf(),
            config.skills.skill_creation.clone(),
        ));
        Some(Arc::new(Self::new(
            creator,
            None,
            config.skills.auto_evolution.clone(),
        )))
    }

    /// Evaluate the trace and, if the gate passes, spawn a background task
    /// that calls [`SkillCreator::create_from_execution`]. Returns the
    /// decision synchronously; the actual skill write happens off the caller's
    /// task.
    ///
    /// `novelty_hint` and `user_signal` come from the caller because they
    /// require external context (embedding index, consolidation LLM output).
    /// Pass `1.0` for novelty when no skill index is available yet.
    pub fn try_trigger(
        &self,
        trace: ExecutionTrace,
        novelty_hint: f64,
        user_signal: Option<UserSignal>,
    ) -> TriggerDecision {
        if !self.config.enabled {
            return TriggerDecision::Disabled;
        }
        if matches!(user_signal, Some(UserSignal::Suppress)) {
            return TriggerDecision::Suppressed;
        }

        let signal = value_signal::evaluate(&trace, novelty_hint, user_signal);
        let score = value_signal::score(&signal);
        if score < self.config.trigger_threshold {
            return TriggerDecision::BelowThreshold;
        }

        // Heavy I/O — embedding similarity probe, fs write — moves off task.
        let creator = self.creator.clone();
        let embedding = self.embedding.clone();
        tokio::spawn(async move {
            let embedding_ref = embedding.as_deref();
            if let Err(err) = creator
                .create_from_execution(&trace.user_message, &trace.tool_calls, embedding_ref)
                .await
            {
                tracing::warn!(
                    turn_id = %trace.turn_id,
                    error = %err,
                    "skill auto-evolution: create_from_execution failed"
                );
            }
        });

        TriggerDecision::Spawned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::creator::ToolCallRecord;
    use naraeclaw_config::schema::SkillCreationConfig;
    use serde_json::json;

    fn call(name: &str) -> ToolCallRecord {
        ToolCallRecord {
            name: name.into(),
            args: json!({}),
        }
    }

    fn baseline_trace() -> ExecutionTrace {
        let mut t = ExecutionTrace::new("turn-1", "deploy production cluster");
        t.record_tool_call(call("shell"));
        t.record_tool_call(call("http_request"));
        t.finalize("Cluster is live");
        t
    }

    fn make_service(enabled: bool, threshold: f64) -> SkillEvolutionService {
        let tmp = tempfile::tempdir().unwrap();
        let creator = Arc::new(SkillCreator::new(
            tmp.path().to_path_buf(),
            SkillCreationConfig {
                enabled: true,
                max_skills: 10,
                similarity_threshold: 0.85,
            },
        ));
        let config = SkillAutoEvolutionConfig {
            enabled,
            trigger_threshold: threshold,
            user_signal_keyword: true,
            user_signal_tool: true,
        };
        // Leak the tempdir so it outlives the test — these tests never write
        // anything (we don't await the spawn), but the SkillCreator stores
        // the path.
        std::mem::forget(tmp);
        SkillEvolutionService::new(creator, None, config)
    }

    #[tokio::test]
    async fn disabled_gate_short_circuits() {
        let svc = make_service(false, 0.6);
        let decision = svc.try_trigger(baseline_trace(), 1.0, None);
        assert_eq!(decision, TriggerDecision::Disabled);
    }

    #[tokio::test]
    async fn suppress_short_circuits() {
        let svc = make_service(true, 0.6);
        let decision = svc.try_trigger(baseline_trace(), 1.0, Some(UserSignal::Suppress));
        assert_eq!(decision, TriggerDecision::Suppressed);
    }

    #[tokio::test]
    async fn below_threshold_does_not_spawn() {
        // novelty 0.1 → score = 0.3 + 0.4*0.1 = 0.34 < 0.6
        let svc = make_service(true, 0.6);
        let decision = svc.try_trigger(baseline_trace(), 0.1, None);
        assert_eq!(decision, TriggerDecision::BelowThreshold);
    }

    #[tokio::test]
    async fn baseline_passes_and_spawns() {
        let svc = make_service(true, 0.6);
        let decision = svc.try_trigger(baseline_trace(), 1.0, None);
        assert_eq!(decision, TriggerDecision::Spawned);
        // Give the spawned task a tick to run; we don't assert on file output
        // (SkillCreator is exercised in its own tests).
        tokio::task::yield_now().await;
    }

    #[tokio::test]
    async fn user_keyword_lifts_low_novelty_past_threshold() {
        // novelty 0.2, keyword bonus 0.5 → 0.3 + 0.4*0.2 + 0.5 = 0.88
        let svc = make_service(true, 0.6);
        let decision = svc.try_trigger(baseline_trace(), 0.2, Some(UserSignal::Keyword));
        assert_eq!(decision, TriggerDecision::Spawned);
    }

    #[tokio::test]
    async fn single_step_below_threshold() {
        let svc = make_service(true, 0.6);
        let mut t = ExecutionTrace::new("turn-1", "echo");
        t.record_tool_call(call("shell"));
        t.finalize("done");
        let decision = svc.try_trigger(t, 1.0, None);
        assert_eq!(decision, TriggerDecision::BelowThreshold);
    }

    fn config_with_evolution(enabled: bool) -> Config {
        let mut config = Config::default();
        config.skills.auto_evolution.enabled = enabled;
        // Ensure the inner SkillCreator is also enabled — `from_config`
        // wires it through unchanged.
        config.skills.skill_creation.enabled = true;
        config
    }

    #[test]
    fn from_config_returns_none_when_gate_disabled() {
        let config = config_with_evolution(false);
        let tmp = tempfile::tempdir().unwrap();
        assert!(SkillEvolutionService::from_config(&config, tmp.path()).is_none());
    }

    #[tokio::test]
    async fn from_config_returns_some_when_gate_enabled() {
        let config = config_with_evolution(true);
        let tmp = tempfile::tempdir().unwrap();
        let svc = SkillEvolutionService::from_config(&config, tmp.path())
            .expect("expected a service when gate is enabled");
        // Reaching the spawn branch from inside the service confirms the
        // gate config plumbed through — we don't need to assert on disk
        // writes here. `try_trigger` calls `tokio::spawn`, so the test must
        // run on a Tokio runtime.
        let decision = svc.try_trigger(baseline_trace(), 1.0, None);
        assert_eq!(decision, TriggerDecision::Spawned);
        tokio::task::yield_now().await;
    }
}
