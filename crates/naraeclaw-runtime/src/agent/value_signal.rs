//! Value scoring for prospective skill creation (ADR-005 M2, Bit 2).
//!
//! Given an [`ExecutionTrace`] (M1) and a few external hints, decides whether
//! the turn was worth turning into a reusable skill. Pure functions only —
//! all I/O lives in the surrounding `skill_evolution` service so this module
//! stays unit-testable without spinning up a memory backend or LLM mock.
//!
//! Decision references: ADR-005 §5.3, D1 (threshold 0.6), D2 (user signal
//! channels).

use serde::{Deserialize, Serialize};

use crate::agent::execution_trace::ExecutionTrace;

/// Explicit user signal from the latest turn. None means "no positive nudge
/// from the user this turn". Negative signals (e.g. "forget that") are
/// represented as `Some(UserSignal::Suppress)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserSignal {
    /// Free-text marker extracted by the consolidation LLM (D2 keyword path).
    Keyword,
    /// Agent invoked the `mark_skill_candidate` tool (D2 tool path).
    Tool,
    /// User asked us to forget this — strictly suppress trigger this turn.
    Suppress,
}

impl UserSignal {
    /// Bonus added to the value score. `Suppress` short-circuits scoring to
    /// zero upstream, so its weight is 0 here for safety.
    pub fn weight(self) -> f64 {
        match self {
            Self::Keyword | Self::Tool => 0.5,
            Self::Suppress => 0.0,
        }
    }
}

/// The five signals fed into [`score`]. Construct via [`evaluate`] from an
/// [`ExecutionTrace`], or by hand in tests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueSignal {
    /// At least two distinct tool calls in the turn.
    pub multistep: bool,
    /// The turn appears to have produced a useful answer (no obvious error
    /// markers in the assistant response, non-trivial length).
    pub succeeded: bool,
    /// `1.0 - max_cosine_similarity(existing skills)`, or `1.0` when there
    /// is no embedding index yet. Higher = more novel.
    pub novelty: f64,
    /// `retry_count / total_calls` clamped to `[0, 1]`. Friction implies the
    /// model worked through trial-and-error — valuable to capture.
    pub friction: f64,
    /// Explicit user nudge, if any.
    pub user_signal: Option<UserSignal>,
}

impl ValueSignal {
    /// Convenience constructor used in tests; not exposed as `Default` so
    /// callers consciously pick values for each field.
    pub fn new(
        multistep: bool,
        succeeded: bool,
        novelty: f64,
        friction: f64,
        user_signal: Option<UserSignal>,
    ) -> Self {
        Self {
            multistep,
            succeeded,
            novelty: novelty.clamp(0.0, 1.0),
            friction: friction.clamp(0.0, 1.0),
            user_signal,
        }
    }
}

/// Compute the trigger score in `[0, 1]`. Pure function.
///
/// Hard preconditions short-circuit to zero:
/// * not multistep → zero
/// * not succeeded → zero
/// * explicit suppression → zero
///
/// Otherwise:
/// `0.3 base + 0.4 * novelty + 0.3 * friction + user_signal.weight()`,
/// clamped to `[0, 1]`.
pub fn score(signal: &ValueSignal) -> f64 {
    if !signal.multistep || !signal.succeeded {
        return 0.0;
    }
    if matches!(signal.user_signal, Some(UserSignal::Suppress)) {
        return 0.0;
    }
    let bonus = signal.user_signal.map(UserSignal::weight).unwrap_or(0.0);
    let raw = 0.3 + 0.4 * signal.novelty + 0.3 * signal.friction + bonus;
    raw.clamp(0.0, 1.0)
}

/// Extract a [`ValueSignal`] from an [`ExecutionTrace`].
///
/// `novelty` is supplied externally (it requires the embedding index, which
/// lives in the memory layer) — pass `1.0` when no index is available yet.
/// `user_signal` is also external because keyword extraction depends on a
/// LLM consolidation pass (M3 work).
pub fn evaluate(
    trace: &ExecutionTrace,
    novelty: f64,
    user_signal: Option<UserSignal>,
) -> ValueSignal {
    let multistep = trace.is_multistep();
    let succeeded = looks_succeeded(trace);
    let friction = trace_friction(trace);
    ValueSignal::new(multistep, succeeded, novelty, friction, user_signal)
}

/// Heuristic: trace looks successful when no explicit failure markers appear
/// in the assistant response and the response carries some content. Cheap,
/// can be replaced by a stronger signal (LLM critic, tool exit codes) later.
fn looks_succeeded(trace: &ExecutionTrace) -> bool {
    if trace.error_count > 0 {
        // Allow recoverable errors as long as the final response is positive;
        // we only block when error_count strictly exceeds retry_count (i.e.
        // an unrecovered failure remains).
        if trace.error_count > trace.retry_count {
            return false;
        }
    }
    let text = trace.assistant_response.trim();
    if text.is_empty() {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    const FAILURE_MARKERS: [&str; 6] = [
        "error:",
        "exception:",
        "traceback",
        "could not",
        "failed to",
        "i was unable",
    ];
    !FAILURE_MARKERS.iter().any(|m| lower.contains(m))
}

/// Friction = retries / (total tool calls), clamped to `[0, 1]`. Zero when
/// there were no tool calls.
fn trace_friction(trace: &ExecutionTrace) -> f64 {
    let total = trace.tool_calls.len();
    if total == 0 {
        return 0.0;
    }
    let retries = f64::from(trace.retry_count);
    (retries / total as f64).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::creator::ToolCallRecord;
    use serde_json::json;

    fn call(name: &str) -> ToolCallRecord {
        ToolCallRecord {
            name: name.into(),
            args: json!({}),
        }
    }

    fn baseline_trace() -> ExecutionTrace {
        let mut t = ExecutionTrace::new("turn-1", "hi");
        t.record_tool_call(call("a"));
        t.record_tool_call(call("b"));
        t.finalize("done");
        t
    }

    #[test]
    fn single_step_scores_zero() {
        let mut t = ExecutionTrace::new("turn-1", "hi");
        t.record_tool_call(call("a"));
        t.finalize("done");
        let s = evaluate(&t, 1.0, None);
        assert!((score(&s) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn failure_marker_in_response_scores_zero() {
        let mut t = baseline_trace();
        t.finalize("Error: nothing to deploy");
        let s = evaluate(&t, 1.0, None);
        assert!((score(&s) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn empty_response_scores_zero() {
        let mut t = baseline_trace();
        t.finalize("");
        let s = evaluate(&t, 1.0, None);
        assert!((score(&s) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn unrecovered_error_scores_zero() {
        let mut t = baseline_trace();
        t.record_error();
        t.record_error();
        t.record_retry();
        // 2 errors, 1 retry → strictly more errors than retries → fail
        let s = evaluate(&t, 1.0, None);
        assert!((score(&s) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn recovered_error_still_succeeds() {
        let mut t = baseline_trace();
        t.record_error();
        t.record_retry();
        let s = evaluate(&t, 1.0, None);
        assert!(score(&s) > 0.0);
        assert!(s.friction > 0.0);
    }

    #[test]
    fn baseline_passes_d1_threshold() {
        // Multistep success with full novelty (no existing skills) but no
        // friction or user signal → 0.3 + 0.4*1.0 = 0.7 ≥ 0.6.
        let t = baseline_trace();
        let s = evaluate(&t, 1.0, None);
        assert!(score(&s) >= 0.6, "got {}", score(&s));
    }

    #[test]
    fn low_novelty_falls_below_threshold() {
        // 0.3 + 0.4*0.2 = 0.38 < 0.6
        let t = baseline_trace();
        let s = evaluate(&t, 0.2, None);
        assert!(score(&s) < 0.6);
    }

    #[test]
    fn user_keyword_boosts_score() {
        let t = baseline_trace();
        let plain = score(&evaluate(&t, 0.2, None));
        let with_keyword = score(&evaluate(&t, 0.2, Some(UserSignal::Keyword)));
        assert!(with_keyword > plain);
        assert!(with_keyword >= 0.6, "keyword should lift past threshold");
    }

    #[test]
    fn suppress_short_circuits_to_zero() {
        let t = baseline_trace();
        let s = evaluate(&t, 1.0, Some(UserSignal::Suppress));
        assert!((score(&s) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn score_is_clamped_to_unit_interval() {
        // novelty=1, friction=1, plus user_signal=0.5 → 0.3+0.4+0.3+0.5 = 1.5
        let signal = ValueSignal::new(true, true, 1.0, 1.0, Some(UserSignal::Tool));
        assert!((score(&signal) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn signal_serde_roundtrip() {
        let s = ValueSignal::new(true, true, 0.7, 0.2, Some(UserSignal::Keyword));
        let json = serde_json::to_string(&s).unwrap();
        let parsed: ValueSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, s);
    }
}
