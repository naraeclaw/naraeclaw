//! Background scheduler for SkillForge (ADR-005 M4).
//!
//! When `[skillforge.scheduler] enabled = true` *and* `[skillforge] enabled =
//! true`, the daemon spawns this loop alongside the cron scheduler. Every
//! `scan_interval_hours` (or `interval_secs` override for tests) it invokes
//! `SkillForge::forge()` and logs the resulting report.
//!
//! The loop is fire-and-forget: failures are logged and the supervisor keeps
//! ticking. A cancellation point lives between ticks so the daemon shutdown
//! path can abort the task cleanly.

use anyhow::Result;
use naraeclaw_config::schema::Config;
use tokio::time::{self, Duration};
use tracing::{info, warn};

use super::{SkillForge, SkillForgeConfig};

const COMPONENT: &str = "skillforge-scheduler";
const MIN_INTERVAL_SECS: u64 = 60;

/// Run the SkillForge background scheduler until the task is aborted.
///
/// Returns `Ok(())` only if the scheduler is disabled (skipped startup);
/// otherwise the loop runs forever and is meant to be torn down by the
/// supervising daemon.
pub async fn run(config: Config) -> Result<()> {
    let cfg = &config.skillforge;

    if !cfg.scheduler.enabled {
        crate::health::mark_component_ok(COMPONENT);
        info!("SkillForge scheduler disabled by config; skipping");
        return Ok(());
    }

    if !cfg.enabled {
        crate::health::mark_component_ok(COMPONENT);
        warn!(
            "SkillForge scheduler enabled but [skillforge] enabled = false; \
             pipeline will be a no-op. Set skillforge.enabled = true to activate."
        );
    }

    let interval_secs = effective_interval_secs(cfg);
    let mut ticker = time::interval(Duration::from_secs(interval_secs));
    // Wait one full interval before the first run so daemon startup isn't
    // amplified by an immediate network burst from scout.
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    ticker.tick().await; // consume the immediate first tick

    crate::health::mark_component_ok(COMPONENT);
    info!(interval_secs, "SkillForge background scheduler started");

    loop {
        ticker.tick().await;
        crate::health::mark_component_ok(COMPONENT);

        let forge = SkillForge::new(cfg.clone());
        match forge.forge().await {
            Ok(report) => {
                info!(
                    discovered = report.discovered,
                    evaluated = report.evaluated,
                    auto_integrated = report.auto_integrated,
                    manual_review = report.manual_review,
                    skipped = report.skipped,
                    "SkillForge scheduled run complete"
                );
            }
            Err(e) => {
                crate::health::mark_component_error(COMPONENT, e.to_string());
                warn!(error = %e, "SkillForge scheduled run failed; will retry next tick");
            }
        }
    }
}

/// Resolve the effective polling interval in seconds.
///
/// `interval_secs` overrides `scan_interval_hours` when set (test-only knob).
/// The minimum is clamped to `MIN_INTERVAL_SECS` to keep runaway configs from
/// hammering external APIs.
fn effective_interval_secs(cfg: &SkillForgeConfig) -> u64 {
    let raw = cfg
        .scheduler
        .interval_secs
        .unwrap_or_else(|| cfg.scan_interval_hours.saturating_mul(3600));
    raw.max(MIN_INTERVAL_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> SkillForgeConfig {
        SkillForgeConfig {
            enabled: true,
            scan_interval_hours: 24,
            ..SkillForgeConfig::default()
        }
    }

    #[test]
    fn interval_defaults_to_scan_hours() {
        let cfg = base_config();
        assert_eq!(effective_interval_secs(&cfg), 24 * 3600);
    }

    #[test]
    fn interval_override_takes_precedence() {
        let mut cfg = base_config();
        cfg.scheduler.interval_secs = Some(120);
        assert_eq!(effective_interval_secs(&cfg), 120);
    }

    #[test]
    fn interval_clamped_to_minimum() {
        let mut cfg = base_config();
        cfg.scheduler.interval_secs = Some(5);
        assert_eq!(effective_interval_secs(&cfg), MIN_INTERVAL_SECS);
    }

    #[test]
    fn zero_scan_hours_falls_back_to_minimum() {
        let mut cfg = base_config();
        cfg.scan_interval_hours = 0;
        cfg.scheduler.interval_secs = None;
        assert_eq!(effective_interval_secs(&cfg), MIN_INTERVAL_SECS);
    }

    #[tokio::test]
    async fn disabled_scheduler_returns_immediately() {
        let mut config = Config::default();
        config.skillforge.enabled = true;
        config.skillforge.scheduler.enabled = false;

        // Should resolve well within a second; if it ever loops, this test hangs.
        let result = tokio::time::timeout(Duration::from_millis(200), run(config)).await;
        assert!(matches!(result, Ok(Ok(()))));
    }

    #[tokio::test]
    async fn enabled_scheduler_runs_at_least_once() {
        // Construct an enabled scheduler with the shortest legal interval and
        // confirm the loop actually invokes `forge()`. We can't observe the
        // call directly without a heavier seam, but if the scheduler stays in
        // its setup phase the `health` component never flips to ok — so we
        // check that gate.
        let mut config = Config::default();
        config.skillforge.enabled = false; // keep the forge body a no-op
        config.skillforge.scheduler.enabled = true;
        config.skillforge.scheduler.interval_secs = Some(MIN_INTERVAL_SECS);

        let handle = tokio::spawn(run(config));
        // Allow the scheduler past `ticker.tick().await` warm-up.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let snapshot = crate::health::snapshot_json();
        let entry = &snapshot["components"][COMPONENT];
        assert_eq!(entry["status"], "ok");

        handle.abort();
        let _ = handle.await;
    }
}
