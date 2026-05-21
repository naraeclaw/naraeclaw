//! SkillForge — Skill auto-discovery, evaluation, and integration engine.
//!
//! Pipeline: Scout → Evaluate → Integrate
//! Discovers skills from external sources, scores them, and generates
//! NaraeClaw-compatible manifests for qualified candidates.

pub mod evaluate;
pub mod injection_guard;
pub mod integrate;
pub mod scheduler;
pub mod scout;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use self::evaluate::{EvalResult, Evaluator, Recommendation};
use self::integrate::Integrator;
use self::scout::{GitHubScout, Scout, ScoutResult, ScoutSource};

// ---------------------------------------------------------------------------
// Configuration — re-exported from naraeclaw-config schema so existing
// callers can keep importing it from this module.
// ---------------------------------------------------------------------------

pub use naraeclaw_config::schema::{SkillForgeConfig, SkillForgeSchedulerConfig};

// ---------------------------------------------------------------------------
// ForgeReport — summary of a single pipeline run
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeReport {
    pub discovered: usize,
    pub evaluated: usize,
    pub auto_integrated: usize,
    pub manual_review: usize,
    pub skipped: usize,
    pub results: Vec<EvalResult>,
}

// ---------------------------------------------------------------------------
// SkillForge
// ---------------------------------------------------------------------------

pub struct SkillForge {
    config: SkillForgeConfig,
    evaluator: Evaluator,
    integrator: Integrator,
}

impl SkillForge {
    pub fn new(config: SkillForgeConfig) -> Self {
        let evaluator = Evaluator::new(config.min_score);
        let integrator = Integrator::new(config.output_dir.clone());
        Self {
            config,
            evaluator,
            integrator,
        }
    }

    /// Run the full pipeline: Scout → Evaluate → Integrate.
    pub async fn forge(&self) -> Result<ForgeReport> {
        if !self.config.enabled {
            warn!("SkillForge is disabled — skipping");
            return Ok(ForgeReport {
                discovered: 0,
                evaluated: 0,
                auto_integrated: 0,
                manual_review: 0,
                skipped: 0,
                results: vec![],
            });
        }

        // --- Scout ----------------------------------------------------------
        let mut candidates: Vec<ScoutResult> = Vec::new();

        for src in &self.config.sources {
            let source: ScoutSource = src.parse().unwrap(); // Infallible
            match source {
                ScoutSource::GitHub => {
                    let scout = GitHubScout::new(self.config.github_token.clone());
                    match scout.discover().await {
                        Ok(mut found) => {
                            info!(count = found.len(), "GitHub scout returned candidates");
                            candidates.append(&mut found);
                        }
                        Err(e) => {
                            warn!(error = %e, "GitHub scout failed, continuing with other sources");
                        }
                    }
                }
                ScoutSource::ClawHub | ScoutSource::HuggingFace => {
                    info!(
                        source = src.as_str(),
                        "Source not yet implemented — skipping"
                    );
                }
            }
        }

        // Deduplicate by URL
        scout::dedup(&mut candidates);
        let discovered = candidates.len();
        info!(discovered, "Total unique candidates after dedup");

        // --- Evaluate -------------------------------------------------------
        let results: Vec<EvalResult> = candidates
            .into_iter()
            .map(|c| self.evaluator.evaluate(c))
            .collect();
        let evaluated = results.len();

        // --- Integrate ------------------------------------------------------
        let mut auto_integrated = 0usize;
        let mut manual_review = 0usize;
        let mut skipped = 0usize;

        for res in &results {
            match res.recommendation {
                Recommendation::Auto => {
                    if self.config.auto_integrate {
                        match self.integrator.integrate(&res.candidate) {
                            Ok(_) => {
                                auto_integrated += 1;
                            }
                            Err(e) => {
                                warn!(
                                    skill = res.candidate.name.as_str(),
                                    error = %e,
                                    "Integration failed for candidate, continuing"
                                );
                            }
                        }
                    } else {
                        // Count as would-be auto but not actually integrated
                        manual_review += 1;
                    }
                }
                Recommendation::Manual => {
                    manual_review += 1;
                }
                Recommendation::Skip => {
                    skipped += 1;
                }
            }
        }

        info!(
            auto_integrated,
            manual_review, skipped, "Forge pipeline complete"
        );

        Ok(ForgeReport {
            discovered,
            evaluated,
            auto_integrated,
            manual_review,
            skipped,
            results,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_forge_returns_empty_report() {
        let cfg = SkillForgeConfig {
            enabled: false,
            ..Default::default()
        };
        let forge = SkillForge::new(cfg);
        let report = forge.forge().await.unwrap();
        assert_eq!(report.discovered, 0);
        assert_eq!(report.auto_integrated, 0);
    }

    #[test]
    fn default_config_values() {
        let cfg = SkillForgeConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.auto_integrate);
        assert_eq!(cfg.scan_interval_hours, 24);
        assert!((cfg.min_score - 0.7).abs() < f64::EPSILON);
        assert_eq!(cfg.sources, vec!["github", "clawhub"]);
    }
}
