//! SkillForge configuration — external skill discovery / evaluation / integration.
//!
//! Used by `naraeclaw-runtime::skillforge::SkillForge` to scout, score, and
//! integrate community skills. The `[skillforge.scheduler]` sub-section was
//! added in ADR-005 M4 to drive periodic background runs.
#![allow(unused_imports)]
use super::*;
use naraeclaw_macros::Configurable;
use serde::{Deserialize, Serialize};

// ── Top-level SkillForge config ─────────────────────────────────

/// Top-level SkillForge configuration (`[skillforge]` section).
///
/// Compatibility: additive and disabled by default — existing configs remain
/// valid when this section is omitted. The forge pipeline only runs when
/// `enabled = true`; the M4 background scheduler additionally requires
/// `[skillforge.scheduler] enabled = true`.
#[derive(Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "skillforge"]
pub struct SkillForgeConfig {
    /// Master toggle. When `false`, `SkillForge::forge()` returns an empty report.
    #[serde(default)]
    pub enabled: bool,

    /// Automatically integrate skills whose evaluator recommendation is `Auto`.
    /// When `false`, every recommendation is downgraded to `Manual`.
    #[serde(default = "default_auto_integrate")]
    pub auto_integrate: bool,

    /// Scout sources to query each run. Recognized values: `github`, `clawhub`,
    /// `huggingface`. Unknown values are skipped.
    #[serde(default = "default_sources")]
    pub sources: Vec<String>,

    /// Interval between background scheduler runs, in hours. Used by the M4
    /// scheduler when `[skillforge.scheduler] enabled = true`.
    #[serde(default = "default_scan_interval")]
    pub scan_interval_hours: u64,

    /// Minimum aggregate score required for `Auto` recommendation.
    #[serde(default = "default_min_score")]
    pub min_score: f64,

    /// Optional GitHub personal-access token for higher rate limits during scout.
    #[serde(default)]
    pub github_token: Option<String>,

    /// Directory where integrated skills are written.
    #[serde(default = "default_output_dir")]
    pub output_dir: String,

    /// Background scheduler sub-section (`[skillforge.scheduler]`).
    #[serde(default)]
    #[nested]
    pub scheduler: SkillForgeSchedulerConfig,
}

fn default_auto_integrate() -> bool {
    true
}
fn default_sources() -> Vec<String> {
    vec!["github".into(), "clawhub".into()]
}
fn default_scan_interval() -> u64 {
    24
}
fn default_min_score() -> f64 {
    0.7
}
fn default_output_dir() -> String {
    "./skills".into()
}

impl Default for SkillForgeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_integrate: default_auto_integrate(),
            sources: default_sources(),
            scan_interval_hours: default_scan_interval(),
            min_score: default_min_score(),
            github_token: None,
            output_dir: default_output_dir(),
            scheduler: SkillForgeSchedulerConfig::default(),
        }
    }
}

impl std::fmt::Debug for SkillForgeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillForgeConfig")
            .field("enabled", &self.enabled)
            .field("auto_integrate", &self.auto_integrate)
            .field("sources", &self.sources)
            .field("scan_interval_hours", &self.scan_interval_hours)
            .field("min_score", &self.min_score)
            .field("github_token", &self.github_token.as_ref().map(|_| "***"))
            .field("output_dir", &self.output_dir)
            .field("scheduler", &self.scheduler)
            .finish()
    }
}

// ── Background scheduler sub-section ───────────────────────────

/// Background scheduler sub-section (`[skillforge.scheduler]`).
///
/// ADR-005 M4. When `enabled = true` *and* `[skillforge] enabled = true`,
/// the daemon spawns a supervisor that runs `SkillForge::forge()` every
/// `scan_interval_hours` (overridable via `interval_secs` for tests).
///
/// Compatibility: additive and disabled by default.
/// Rollback/migration: omit the section or set `enabled = false`.
#[derive(Debug, Default, Clone, Serialize, Deserialize, Configurable)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "skillforge.scheduler"]
pub struct SkillForgeSchedulerConfig {
    /// Enable the background scheduler. Default `false` (M4 ships off).
    #[serde(default)]
    pub enabled: bool,

    /// Optional override of the run interval in seconds. When `None`, the
    /// scheduler uses `scan_interval_hours * 3600`. Test-only knob — production
    /// configs should leave this unset and tune `scan_interval_hours` instead.
    #[serde(default)]
    pub interval_secs: Option<u64>,
}
