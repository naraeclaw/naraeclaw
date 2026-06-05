//! Per-skill usage statistics persisted in the memory layer
//! (ADR-005 M1, Bit 6).
//!
//! Records are stored as a normal [`MemoryEntry`] under the
//! `Custom("skill_stat")` category, keyed by skill slug. Storing through the
//! memory layer (rather than a bespoke table) means decay and namespace
//! isolation apply for free, so unused skills naturally fade out per ADR
//! decision D4 ("decay-based natural retirement").

use anyhow::Result;
use chrono::{DateTime, Utc};
use naraeclaw_memory::{Memory, MemoryCategory};
use serde::{Deserialize, Serialize};

/// JSON payload persisted in `MemoryEntry::content` for one skill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillStatRecord {
    pub slug: String,
    pub invocations: u32,
    pub successes: u32,
    pub failures: u32,
    /// RFC 3339 timestamp of the most recent invocation.
    pub last_used_at: String,
}

impl SkillStatRecord {
    /// Build a record from a prior snapshot (if any) plus a new outcome.
    /// Pure function — extracted so unit tests don't need a memory backend.
    pub fn updated(
        prev: Option<&SkillStatRecord>,
        slug: &str,
        success: bool,
        now: DateTime<Utc>,
    ) -> Self {
        let base = prev.cloned().unwrap_or_else(|| SkillStatRecord {
            slug: slug.to_string(),
            invocations: 0,
            successes: 0,
            failures: 0,
            last_used_at: now.to_rfc3339(),
        });
        SkillStatRecord {
            slug: base.slug,
            invocations: base.invocations.saturating_add(1),
            successes: base.successes.saturating_add(if success { 1 } else { 0 }),
            failures: base.failures.saturating_add(if success { 0 } else { 1 }),
            last_used_at: now.to_rfc3339(),
        }
    }

    /// Failure rate over total invocations, or `None` when there are no
    /// recorded invocations yet.
    pub fn failure_rate(&self) -> Option<f64> {
        if self.invocations == 0 {
            None
        } else {
            Some(f64::from(self.failures) / f64::from(self.invocations))
        }
    }
}

/// Memory category bucket used for every record this module persists.
pub fn skill_stat_category() -> MemoryCategory {
    MemoryCategory::Custom("skill_stat".to_string())
}

/// Stable memory key for a given skill slug.
pub fn skill_stat_key(slug: &str) -> String {
    format!("skill_stat::{slug}")
}

const SKILL_STAT_IMPORTANCE: f64 = 0.4;

/// Record one invocation of `slug` against the memory backend.
///
/// Reads any prior record under [`skill_stat_key`], merges the new outcome,
/// and writes the result back through [`Memory::store_with_metadata`] with
/// the caller's `namespace` and a fixed importance of `0.4`.
///
/// Not atomic across concurrent invocations of the same slug — a follow-up
/// milestone may add a per-slug mutex if contention shows up in practice.
pub async fn record_invocation(
    memory: &dyn Memory,
    namespace: &str,
    slug: &str,
    success: bool,
) -> Result<SkillStatRecord> {
    record_invocation_at(memory, namespace, slug, success, Utc::now()).await
}

/// [`record_invocation`] with an explicit timestamp — used in tests so that
/// the resulting record is deterministic.
pub async fn record_invocation_at(
    memory: &dyn Memory,
    namespace: &str,
    slug: &str,
    success: bool,
    now: DateTime<Utc>,
) -> Result<SkillStatRecord> {
    let key = skill_stat_key(slug);
    let prev_entry = memory.get(&key).await?;
    let prev_record = prev_entry
        .as_ref()
        .and_then(|e| serde_json::from_str::<SkillStatRecord>(&e.content).ok());
    let next = SkillStatRecord::updated(prev_record.as_ref(), slug, success, now);
    let content = serde_json::to_string(&next)?;
    memory
        .store_with_metadata(
            &key,
            &content,
            skill_stat_category(),
            None,
            Some(namespace),
            Some(SKILL_STAT_IMPORTANCE),
        )
        .await?;
    Ok(next)
}

// ── Skill semantic index (ADR-005 M3b, Bit 5 reverse bridge, D3) ────────────

/// Memory category bucket for the skill semantic index. Kept separate from
/// `skill_stat` so recall filters and decay tuning stay independent (D3).
pub fn skill_index_category() -> MemoryCategory {
    MemoryCategory::Custom("skill_index".to_string())
}

/// Stable memory key for a given skill slug's index entry.
pub fn skill_index_key(slug: &str) -> String {
    format!("skill_index::{slug}")
}

/// Higher than `skill_stat` (0.4) so a freshly created skill surfaces in
/// natural-language recall (ADR-005 D3 chose importance 0.8).
const SKILL_INDEX_IMPORTANCE: f64 = 0.8;

/// Persist a semantic index entry for a newly created skill so that a user's
/// natural-language query can surface the skill via `recall` (ADR-005 M3b,
/// Bit 5 reverse bridge). Idempotent per slug (stable key overwrites).
///
/// `summary` is rendered into natural-language content so the memory layer's
/// FTS/vector retrieval can match it; the slug is included for traceability.
pub async fn store_skill_index(memory: &dyn Memory, slug: &str, summary: &str) -> Result<()> {
    let key = skill_index_key(slug);
    let content = format!("Skill `{slug}`: {}", summary.trim());
    memory
        .store_with_metadata(
            &key,
            &content,
            skill_index_category(),
            None,
            None,
            Some(SKILL_INDEX_IMPORTANCE),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::TimeZone;
    use naraeclaw_memory::MemoryEntry;
    use std::sync::Mutex;

    #[test]
    fn updated_from_none_starts_counters_at_one_success() {
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap();
        let r = SkillStatRecord::updated(None, "deploy", true, now);
        assert_eq!(r.slug, "deploy");
        assert_eq!(r.invocations, 1);
        assert_eq!(r.successes, 1);
        assert_eq!(r.failures, 0);
        assert_eq!(r.last_used_at, now.to_rfc3339());
    }

    #[test]
    fn updated_accumulates_failure() {
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap();
        let later = Utc.with_ymd_and_hms(2026, 5, 13, 0, 1, 0).unwrap();
        let r1 = SkillStatRecord::updated(None, "deploy", true, now);
        let r2 = SkillStatRecord::updated(Some(&r1), "deploy", false, later);
        assert_eq!(r2.invocations, 2);
        assert_eq!(r2.successes, 1);
        assert_eq!(r2.failures, 1);
        assert_eq!(r2.last_used_at, later.to_rfc3339());
    }

    #[test]
    fn failure_rate_handles_empty_record() {
        let mut r = SkillStatRecord::updated(None, "x", true, Utc::now());
        assert!((r.failure_rate().unwrap() - 0.0).abs() < f64::EPSILON);
        r.invocations = 0;
        r.successes = 0;
        r.failures = 0;
        assert!(r.failure_rate().is_none());
    }

    #[test]
    fn skill_stat_key_uses_namespaced_prefix() {
        assert_eq!(skill_stat_key("deploy"), "skill_stat::deploy");
    }

    #[test]
    fn skill_stat_category_is_custom_skill_stat() {
        assert_eq!(
            skill_stat_category(),
            MemoryCategory::Custom("skill_stat".to_string())
        );
    }

    /// Tiny in-memory `Memory` impl backing the integration test below.
    /// Stores only what `record_invocation_at` actually touches.
    struct InMemory {
        store: Mutex<Vec<MemoryEntry>>,
    }

    impl InMemory {
        fn new() -> Self {
            Self {
                store: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl Memory for InMemory {
        fn name(&self) -> &str {
            "in-memory"
        }

        async fn store(
            &self,
            key: &str,
            content: &str,
            category: MemoryCategory,
            session_id: Option<&str>,
        ) -> anyhow::Result<()> {
            self.store_with_metadata(key, content, category, session_id, None, None)
                .await
        }

        async fn recall(
            &self,
            _query: &str,
            _limit: usize,
            _session_id: Option<&str>,
            _since: Option<&str>,
            _until: Option<&str>,
        ) -> anyhow::Result<Vec<MemoryEntry>> {
            Ok(Vec::new())
        }

        async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>> {
            Ok(self
                .store
                .lock()
                .unwrap()
                .iter()
                .find(|e| e.key == key)
                .cloned())
        }

        async fn list(
            &self,
            _category: Option<&MemoryCategory>,
            _session_id: Option<&str>,
        ) -> anyhow::Result<Vec<MemoryEntry>> {
            Ok(self.store.lock().unwrap().clone())
        }

        async fn forget(&self, _key: &str) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn count(&self) -> anyhow::Result<usize> {
            Ok(self.store.lock().unwrap().len())
        }

        async fn health_check(&self) -> bool {
            true
        }

        async fn store_with_metadata(
            &self,
            key: &str,
            content: &str,
            category: MemoryCategory,
            session_id: Option<&str>,
            namespace: Option<&str>,
            importance: Option<f64>,
        ) -> anyhow::Result<()> {
            let mut guard = self.store.lock().unwrap();
            guard.retain(|e| e.key != key);
            guard.push(MemoryEntry {
                id: key.to_string(),
                key: key.to_string(),
                content: content.to_string(),
                category,
                timestamp: Utc::now().to_rfc3339(),
                session_id: session_id.map(|s| s.to_string()),
                score: None,
                namespace: namespace.unwrap_or("default").to_string(),
                importance,
                superseded_by: None,
            });
            Ok(())
        }
    }

    #[tokio::test]
    async fn record_invocation_at_persists_and_accumulates() {
        let mem = InMemory::new();
        let now = Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap();

        let r1 = record_invocation_at(&mem, "ns-a", "deploy", true, now)
            .await
            .unwrap();
        assert_eq!(r1.invocations, 1);
        assert_eq!(r1.successes, 1);

        let stored = mem
            .get(&skill_stat_key("deploy"))
            .await
            .unwrap()
            .expect("entry must exist after first record");
        assert_eq!(stored.category, skill_stat_category());
        assert_eq!(stored.namespace, "ns-a");
        assert_eq!(stored.importance, Some(SKILL_STAT_IMPORTANCE));
        let parsed: SkillStatRecord = serde_json::from_str(&stored.content).unwrap();
        assert_eq!(parsed, r1);

        let later = Utc.with_ymd_and_hms(2026, 5, 13, 0, 1, 0).unwrap();
        let r2 = record_invocation_at(&mem, "ns-a", "deploy", false, later)
            .await
            .unwrap();
        assert_eq!(r2.invocations, 2);
        assert_eq!(r2.successes, 1);
        assert_eq!(r2.failures, 1);
    }

    #[test]
    fn skill_index_key_and_category_distinct_from_stat() {
        assert_eq!(skill_index_key("deploy"), "skill_index::deploy");
        assert_eq!(
            skill_index_category(),
            MemoryCategory::Custom("skill_index".to_string())
        );
        assert_ne!(skill_index_category(), skill_stat_category());
    }

    #[tokio::test]
    async fn store_skill_index_persists_with_category_and_importance() {
        let mem = InMemory::new();
        store_skill_index(
            &mem,
            "deploy-prod",
            "Deploy the app to production via terraform",
        )
        .await
        .unwrap();

        let stored = mem
            .get(&skill_index_key("deploy-prod"))
            .await
            .unwrap()
            .expect("skill_index entry must exist after store");
        assert_eq!(stored.category, skill_index_category());
        assert_eq!(stored.importance, Some(SKILL_INDEX_IMPORTANCE));
        assert!(stored.content.contains("deploy-prod"), "slug in content");
        assert!(stored.content.contains("terraform"), "summary in content");
    }
}
