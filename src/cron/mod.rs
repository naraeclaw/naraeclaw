pub use naraeclaw_runtime::cron::*;

use crate::config::Config;
use anyhow::{Result, bail};

/// `Vec<String>` → `Option<Vec<String>>` (빈 경우 None)
fn to_optional_tools(v: Vec<String>) -> Option<Vec<String>> {
    if v.is_empty() { None } else { Some(v) }
}

/// shell 작업에서 --allowed-tool 사용 시 에러
fn bail_if_tools_without_agent(allowed_tools: &[String]) -> anyhow::Result<()> {
    if !allowed_tools.is_empty() {
        anyhow::bail!("--allowed-tool is only supported with --agent cron jobs");
    }
    Ok(())
}

pub fn handle_command(command: crate::CronCommands, config: &Config) -> Result<()> {
    match command {
        crate::CronCommands::List => {
            let jobs = list_jobs(config)?;
            if jobs.is_empty() {
                println!("No scheduled tasks yet.");
                println!("\nUsage:");
                println!("  naraeclaw cron add '0 9 * * *' 'agent -m \"Good morning!\"'");
                return Ok(());
            }

            println!("🕒 Scheduled jobs ({}):", jobs.len());
            for job in jobs {
                let last_run = job
                    .last_run
                    .map_or_else(|| "never".into(), |d| d.to_rfc3339());
                let last_status = job.last_status.unwrap_or_else(|| "n/a".into());
                println!(
                    "- {} | {:?} | next={} | last={} ({})",
                    job.id,
                    job.schedule,
                    job.next_run.to_rfc3339(),
                    last_run,
                    last_status,
                );
                if !job.command.is_empty() {
                    println!("    cmd: {}", job.command);
                }
                if let Some(prompt) = &job.prompt {
                    println!("    prompt: {prompt}");
                }
            }
            Ok(())
        }
        crate::CronCommands::Add {
            expression,
            tz,
            agent,
            allowed_tools,
            command,
        } => {
            let schedule = Schedule::Cron {
                expr: expression,
                tz,
            };
            if agent {
                let job = add_agent_job(
                    config,
                    None,
                    schedule,
                    &command,
                    SessionTarget::Isolated,
                    None,
                    None,
                    false,
                    to_optional_tools(allowed_tools),
                )?;
                println!("✅ Added agent cron job {}", job.id);
                println!("  Expr  : {}", job.expression);
                println!("  Next  : {}", job.next_run.to_rfc3339());
                println!("  Prompt: {}", job.prompt.as_deref().unwrap_or_default());
            } else {
                bail_if_tools_without_agent(&allowed_tools)?;
                let job = add_shell_job(config, None, schedule, &command)?;
                println!("✅ Added cron job {}", job.id);
                println!("  Expr: {}", job.expression);
                println!("  Next: {}", job.next_run.to_rfc3339());
                println!("  Cmd : {}", job.command);
            }
            Ok(())
        }
        crate::CronCommands::AddAt {
            at,
            agent,
            allowed_tools,
            command,
        } => {
            let at = chrono::DateTime::parse_from_rfc3339(&at)
                .map_err(|e| anyhow::anyhow!("Invalid RFC3339 timestamp for --at: {e}"))?
                .with_timezone(&chrono::Utc);
            let schedule = Schedule::At { at };
            if agent {
                let job = add_agent_job(
                    config,
                    None,
                    schedule,
                    &command,
                    SessionTarget::Isolated,
                    None,
                    None,
                    true,
                    to_optional_tools(allowed_tools),
                )?;
                println!("✅ Added one-shot agent cron job {}", job.id);
                println!("  At    : {}", job.next_run.to_rfc3339());
                println!("  Prompt: {}", job.prompt.as_deref().unwrap_or_default());
            } else {
                bail_if_tools_without_agent(&allowed_tools)?;
                let job = add_shell_job(config, None, schedule, &command)?;
                println!("✅ Added one-shot cron job {}", job.id);
                println!("  At  : {}", job.next_run.to_rfc3339());
                println!("  Cmd : {}", job.command);
            }
            Ok(())
        }
        crate::CronCommands::AddEvery {
            every_ms,
            agent,
            allowed_tools,
            command,
        } => {
            let schedule = Schedule::Every { every_ms };
            if agent {
                let job = add_agent_job(
                    config,
                    None,
                    schedule,
                    &command,
                    SessionTarget::Isolated,
                    None,
                    None,
                    false,
                    to_optional_tools(allowed_tools),
                )?;
                println!("✅ Added interval agent cron job {}", job.id);
                println!("  Every(ms): {every_ms}");
                println!("  Next     : {}", job.next_run.to_rfc3339());
                println!("  Prompt   : {}", job.prompt.as_deref().unwrap_or_default());
            } else {
                bail_if_tools_without_agent(&allowed_tools)?;
                let job = add_shell_job(config, None, schedule, &command)?;
                println!("✅ Added interval cron job {}", job.id);
                println!("  Every(ms): {every_ms}");
                println!("  Next     : {}", job.next_run.to_rfc3339());
                println!("  Cmd      : {}", job.command);
            }
            Ok(())
        }
        crate::CronCommands::Once {
            delay,
            agent,
            allowed_tools,
            command,
        } => {
            if agent {
                let duration = parse_delay(&delay)?;
                let at = chrono::Utc::now() + duration;
                let schedule = Schedule::At { at };
                let job = add_agent_job(
                    config,
                    None,
                    schedule,
                    &command,
                    SessionTarget::Isolated,
                    None,
                    None,
                    true,
                    to_optional_tools(allowed_tools),
                )?;
                println!("✅ Added one-shot agent cron job {}", job.id);
                println!("  At    : {}", job.next_run.to_rfc3339());
                println!("  Prompt: {}", job.prompt.as_deref().unwrap_or_default());
            } else {
                bail_if_tools_without_agent(&allowed_tools)?;
                let job = add_once(config, &delay, &command)?;
                println!("✅ Added one-shot cron job {}", job.id);
                println!("  At  : {}", job.next_run.to_rfc3339());
                println!("  Cmd : {}", job.command);
            }
            Ok(())
        }
        crate::CronCommands::Update {
            id,
            expression,
            tz,
            command,
            name,
            allowed_tools,
        } => {
            if expression.is_none()
                && tz.is_none()
                && command.is_none()
                && name.is_none()
                && allowed_tools.is_empty()
            {
                bail!(
                    "At least one of --expression, --tz, --command, --name, or --allowed-tool must be provided"
                );
            }

            let existing = if expression.is_some() || tz.is_some() || !allowed_tools.is_empty() {
                Some(get_job(config, &id)?)
            } else {
                None
            };

            // Merge expression/tz with the existing schedule so that
            // --tz alone updates the timezone and --expression alone
            // preserves the existing timezone.
            let schedule = if expression.is_some() || tz.is_some() {
                let existing = existing
                    .as_ref()
                    .expect("existing job must be loaded when updating schedule");
                let (existing_expr, existing_tz) = match &existing.schedule {
                    Schedule::Cron {
                        expr,
                        tz: existing_tz,
                    } => (expr.clone(), existing_tz.clone()),
                    _ => bail!("Cannot update expression/tz on a non-cron schedule"),
                };
                Some(Schedule::Cron {
                    expr: expression.unwrap_or(existing_expr),
                    tz: tz.or(existing_tz),
                })
            } else {
                None
            };

            if !allowed_tools.is_empty() {
                let existing = existing
                    .as_ref()
                    .expect("existing job must be loaded when updating allowed tools");
                if existing.job_type != JobType::Agent {
                    bail!("--allowed-tool is only supported for agent cron jobs");
                }
            }

            let patch = CronJobPatch {
                schedule,
                command,
                name,
                allowed_tools: to_optional_tools(allowed_tools),
                ..CronJobPatch::default()
            };

            let job = update_shell_job_with_approval(config, &id, patch, false)?;
            println!("\u{2705} Updated cron job {}", job.id);
            println!("  Expr: {}", job.expression);
            println!("  Next: {}", job.next_run.to_rfc3339());
            println!("  Cmd : {}", job.command);
            Ok(())
        }
        crate::CronCommands::Remove { id } => remove_job(config, &id),
        crate::CronCommands::Pause { id } => {
            pause_job(config, &id)?;
            println!("⏸️  Paused cron job {id}");
            Ok(())
        }
        crate::CronCommands::Resume { id } => {
            resume_job(config, &id)?;
            println!("▶️  Resumed cron job {id}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(tmp: &TempDir) -> crate::config::Config {
        let mut config = crate::config::Config::default();
        config.workspace_dir = tmp.path().join("workspace");
        config.config_path = tmp.path().join("config.toml");
        std::fs::create_dir_all(&config.workspace_dir).unwrap();
        config
    }

    // to_optional_tools 테스트
    #[test]
    fn to_optional_tools_empty_returns_none() {
        assert!(to_optional_tools(vec![]).is_none());
    }

    #[test]
    fn to_optional_tools_non_empty_returns_some() {
        assert_eq!(to_optional_tools(vec!["shell".into()]), Some(vec!["shell".into()]));
    }

    // bail_if_tools_without_agent 테스트
    #[test]
    fn bail_if_tools_without_agent_empty_ok() {
        assert!(bail_if_tools_without_agent(&[]).is_ok());
    }

    #[test]
    fn bail_if_tools_without_agent_non_empty_errors() {
        assert!(bail_if_tools_without_agent(&["shell".to_string()]).is_err());
    }

    // List 빈 상태
    #[test]
    fn list_empty_jobs() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(&tmp);
        let result = handle_command(crate::CronCommands::List, &config);
        assert!(result.is_ok());
    }

    // Add shell job
    #[test]
    fn add_shell_cron_job() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(&tmp);
        let result = handle_command(
            crate::CronCommands::Add {
                expression: "0 9 * * *".into(),
                tz: None,
                agent: false,
                allowed_tools: vec![],
                command: "echo hello".into(),
            },
            &config,
        );
        assert!(result.is_ok());
    }

    // Add shell job + allowed_tools → error
    #[test]
    fn add_shell_with_tools_errors() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(&tmp);
        let result = handle_command(
            crate::CronCommands::Add {
                expression: "0 9 * * *".into(),
                tz: None,
                agent: false,
                allowed_tools: vec!["shell".into()],
                command: "echo hello".into(),
            },
            &config,
        );
        assert!(result.is_err());
    }

    // AddEvery shell job
    #[test]
    fn add_every_shell_job() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(&tmp);
        let result = handle_command(
            crate::CronCommands::AddEvery {
                every_ms: 60_000,
                agent: false,
                allowed_tools: vec![],
                command: "echo ping".into(),
            },
            &config,
        );
        assert!(result.is_ok());
    }

    // AddAt invalid timestamp
    #[test]
    fn add_at_invalid_timestamp_errors() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(&tmp);
        let result = handle_command(
            crate::CronCommands::AddAt {
                at: "not-a-date".into(),
                agent: false,
                allowed_tools: vec![],
                command: "echo hi".into(),
            },
            &config,
        );
        assert!(result.is_err());
    }

    // Update no fields → error
    #[test]
    fn update_no_fields_errors() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(&tmp);
        let result = handle_command(
            crate::CronCommands::Update {
                id: "nonexistent".into(),
                expression: None,
                tz: None,
                command: None,
                name: None,
                allowed_tools: vec![],
            },
            &config,
        );
        assert!(result.is_err());
    }

    // Remove nonexistent → error
    #[test]
    fn remove_nonexistent_job_errors() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(&tmp);
        let result = handle_command(
            crate::CronCommands::Remove { id: "nonexistent-id".into() },
            &config,
        );
        assert!(result.is_err());
    }

    // Add then list → 1 job
    #[test]
    fn add_then_list_shows_job() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(&tmp);
        handle_command(
            crate::CronCommands::Add {
                expression: "0 9 * * *".into(),
                tz: None,
                agent: false,
                allowed_tools: vec![],
                command: "echo hi".into(),
            },
            &config,
        ).unwrap();
        let jobs = list_jobs(&config).unwrap();
        assert_eq!(jobs.len(), 1);
    }

    // Pause/Resume nonexistent → error
    #[test]
    fn pause_nonexistent_errors() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(&tmp);
        assert!(handle_command(crate::CronCommands::Pause { id: "bad-id".into() }, &config).is_err());
    }

    #[test]
    fn resume_nonexistent_errors() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(&tmp);
        assert!(handle_command(crate::CronCommands::Resume { id: "bad-id".into() }, &config).is_err());
    }
}
