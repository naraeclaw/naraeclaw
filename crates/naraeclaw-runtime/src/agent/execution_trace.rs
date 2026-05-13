//! Per-turn execution trace capture (ADR-005 M1).
//!
//! [`ExecutionTrace`] accumulates the tool-call sequence and outcome of a
//! single agent turn. Downstream stages introduced in later milestones
//! ([`ValueSignal`] in M2, [`SkillCreator`] gate in M2, consolidation
//! bridge in M3) consume the trace; M1 only provides the type, accumulation
//! API, and config gate. Loop wiring is intentionally deferred.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::skills::creator::ToolCallRecord;

/// A structured snapshot of one agent turn.
///
/// Created at turn start, fed `record_tool_call` per dispatched tool, and
/// closed with `finalize` once the assistant response is ready. All fields
/// are public so that downstream evaluators can read them without going
/// through accessor boilerplate; treat construction strictly through the
/// provided builder methods to keep timestamps consistent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub turn_id: String,
    pub user_message: String,
    pub tool_calls: Vec<ToolCallRecord>,
    pub assistant_response: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_count: u32,
    pub retry_count: u32,
}

impl ExecutionTrace {
    /// Start a new trace at turn boundary.
    pub fn new(turn_id: impl Into<String>, user_message: impl Into<String>) -> Self {
        Self::new_at(turn_id, user_message, Utc::now())
    }

    /// Start a new trace with an explicit start instant. Useful for tests.
    pub fn new_at(
        turn_id: impl Into<String>,
        user_message: impl Into<String>,
        started_at: DateTime<Utc>,
    ) -> Self {
        Self {
            turn_id: turn_id.into(),
            user_message: user_message.into(),
            tool_calls: Vec::new(),
            assistant_response: String::new(),
            started_at,
            finished_at: None,
            error_count: 0,
            retry_count: 0,
        }
    }

    /// Append a tool call observation in execution order.
    pub fn record_tool_call(&mut self, call: ToolCallRecord) {
        self.tool_calls.push(call);
    }

    /// Increment the per-turn error counter (a tool returned `Err` or the
    /// response embedded an explicit failure marker).
    pub fn record_error(&mut self) {
        self.error_count = self.error_count.saturating_add(1);
    }

    /// Increment the retry counter (same tool re-invoked after an earlier
    /// attempt). Used as a "friction" signal for value scoring.
    pub fn record_retry(&mut self) {
        self.retry_count = self.retry_count.saturating_add(1);
    }

    /// Seal the trace with the final assistant text and now-ish timestamp.
    pub fn finalize(&mut self, assistant_response: impl Into<String>) {
        self.finalize_at(assistant_response, Utc::now());
    }

    /// `finalize` variant taking an explicit closing instant.
    pub fn finalize_at(
        &mut self,
        assistant_response: impl Into<String>,
        finished_at: DateTime<Utc>,
    ) {
        self.assistant_response = assistant_response.into();
        self.finished_at = Some(finished_at);
    }

    /// Duration in milliseconds, available only after [`Self::finalize`].
    pub fn duration_ms(&self) -> Option<i64> {
        self.finished_at
            .map(|end| (end - self.started_at).num_milliseconds())
    }

    /// True iff the trace covers more than one tool call. This is the cheap
    /// pre-filter used by M2 before computing a full `ValueSignal`.
    pub fn is_multistep(&self) -> bool {
        self.tool_calls.len() >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn sample_call(name: &str) -> ToolCallRecord {
        ToolCallRecord {
            name: name.into(),
            args: json!({"k": "v"}),
        }
    }

    #[test]
    fn new_initialises_empty_trace() {
        let trace = ExecutionTrace::new("turn-1", "hi");
        assert_eq!(trace.turn_id, "turn-1");
        assert_eq!(trace.user_message, "hi");
        assert!(trace.tool_calls.is_empty());
        assert_eq!(trace.error_count, 0);
        assert_eq!(trace.retry_count, 0);
        assert!(trace.finished_at.is_none());
        assert!(!trace.is_multistep());
    }

    #[test]
    fn record_tool_calls_preserves_order() {
        let mut trace = ExecutionTrace::new("turn-1", "do it");
        trace.record_tool_call(sample_call("a"));
        trace.record_tool_call(sample_call("b"));
        trace.record_tool_call(sample_call("c"));
        assert_eq!(trace.tool_calls.len(), 3);
        assert_eq!(trace.tool_calls[0].name, "a");
        assert_eq!(trace.tool_calls[2].name, "c");
        assert!(trace.is_multistep());
    }

    #[test]
    fn finalize_seals_response_and_duration() {
        let started = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 0).unwrap();
        let finished = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 2).unwrap();
        let mut trace = ExecutionTrace::new_at("t", "u", started);
        trace.finalize_at("ok", finished);
        assert_eq!(trace.assistant_response, "ok");
        assert_eq!(trace.duration_ms(), Some(2_000));
    }

    #[test]
    fn counters_saturate_at_max() {
        let mut trace = ExecutionTrace::new("turn-1", "");
        trace.error_count = u32::MAX - 1;
        trace.record_error();
        trace.record_error();
        assert_eq!(trace.error_count, u32::MAX);

        trace.retry_count = u32::MAX;
        trace.record_retry();
        assert_eq!(trace.retry_count, u32::MAX);
    }

    #[test]
    fn serde_roundtrip_preserves_fields() {
        let started = Utc.with_ymd_and_hms(2026, 5, 13, 10, 0, 0).unwrap();
        let mut trace = ExecutionTrace::new_at("t", "u", started);
        trace.record_tool_call(sample_call("a"));
        trace.record_error();
        trace.finalize_at("done", started);

        let json = serde_json::to_string(&trace).unwrap();
        let parsed: ExecutionTrace = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.turn_id, trace.turn_id);
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.error_count, 1);
        assert_eq!(parsed.assistant_response, "done");
    }
}
