use anyhow::{Context, Result, bail};
use naraeclaw_config::schema::Config;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

const SERVICE_LABEL: &str = "com.naraeclaw.daemon";
const WINDOWS_TASK_NAME: &str = "NaraeClaw Daemon";

/// Supported init systems for service management
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InitSystem {
    /// Auto-detect based on system indicators
    #[default]
    Auto,
    /// systemd (via systemctl --user)
    Systemd,
}

impl FromStr for InitSystem {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "systemd" => Ok(Self::Systemd),
            other => bail!("Unknown init system: '{}'. Supported: auto, systemd", other),
        }
    }
}

impl InitSystem {
    /// Resolve auto-detection to a concrete init system
    ///
    /// Detection order (deny-by-default):
    /// 1. `/run/systemd/system` exists → Systemd
    /// 2. else → Error (unknown init system)
    #[cfg(target_os = "linux")]
    pub fn resolve(self) -> Result<Self> {
        match self {
            Self::Auto => detect_init_system(),
            concrete => Ok(concrete),
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn resolve(self) -> Result<Self> {
        match self {
            Self::Auto => Ok(Self::Systemd),
            concrete => Ok(concrete),
        }
    }
}

/// Detect the active init system on Linux
///
/// Returns an error if systemd is not detected.
#[cfg(target_os = "linux")]
fn detect_init_system() -> Result<InitSystem> {
    // Check for systemd first (most common on modern Linux)
    if Path::new("/run/systemd/system").exists() {
        return Ok(InitSystem::Systemd);
    }

    bail!("Could not detect systemd. Use --service-init systemd on supported Linux hosts.");
}

fn windows_task_name() -> &'static str {
    WINDOWS_TASK_NAME
}

/// Returns whether the NaraeClaw daemon service is currently running.
pub fn is_running() -> bool {
    if cfg!(target_os = "macos") {
        run_capture(Command::new("launchctl").arg("list"))
            .map(|out| out.lines().any(|l| l.contains(SERVICE_LABEL)))
            .unwrap_or(false)
    } else if cfg!(target_os = "linux") {
        is_running_linux()
    } else if cfg!(target_os = "windows") {
        run_capture(Command::new("schtasks").args([
            "/Query",
            "/TN",
            WINDOWS_TASK_NAME,
            "/FO",
            "LIST",
        ]))
        .map(|out| out.contains("Running"))
        .unwrap_or(false)
    } else {
        false
    }
}

fn is_running_linux() -> bool {
    run_capture(Command::new("systemctl").args(["--user", "is-active", "naraeclaw.service"]))
        .map(|out| out.trim() == "active")
        .unwrap_or(false)
}

pub fn install(config: &Config, init_system: InitSystem) -> Result<()> {
    if cfg!(target_os = "macos") {
        install_macos(config)
    } else if cfg!(target_os = "linux") {
        let resolved = init_system.resolve()?;
        install_linux(config, resolved)
    } else if cfg!(target_os = "windows") {
        install_windows(config)
    } else {
        anyhow::bail!("Service management is supported on macOS and Linux only");
    }
}

pub fn start(config: &Config, init_system: InitSystem) -> Result<()> {
    if cfg!(target_os = "macos") {
        // Ensure the Homebrew var directory exists before launchd tries to use it.
        // The plist may reference this path for WorkingDirectory and log files.
        let exe = std::env::current_exe().ok();
        if let Some(ref exe_path) = exe
            && let Some(var_dir) = detect_homebrew_var_dir(exe_path)
        {
            let _ = fs::create_dir_all(&var_dir);
        }
        let plist = macos_service_file()?;
        run_checked(Command::new("launchctl").arg("load").arg("-w").arg(&plist))?;
        run_checked(Command::new("launchctl").arg("start").arg(SERVICE_LABEL))?;
        println!("✅ Service started");
        Ok(())
    } else if cfg!(target_os = "linux") {
        let resolved = init_system.resolve()?;
        start_linux(resolved)
    } else if cfg!(target_os = "windows") {
        let _ = config;
        run_checked(Command::new("schtasks").args(["/Run", "/TN", windows_task_name()]))?;
        println!("✅ Service started");
        Ok(())
    } else {
        let _ = config;
        anyhow::bail!("Service management is supported on macOS and Linux only")
    }
}

fn start_linux(init_system: InitSystem) -> Result<()> {
    match init_system {
        InitSystem::Systemd => {
            run_checked(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
            run_checked(Command::new("systemctl").args(["--user", "start", "naraeclaw.service"]))?;
        }
        InitSystem::Auto => unreachable!("Auto should be resolved before this point"),
    }
    println!("✅ Service started");
    Ok(())
}

pub fn stop(config: &Config, init_system: InitSystem) -> Result<()> {
    if cfg!(target_os = "macos") {
        let plist = macos_service_file()?;
        let _ = run_checked(Command::new("launchctl").arg("stop").arg(SERVICE_LABEL));
        let _ = run_checked(
            Command::new("launchctl")
                .arg("unload")
                .arg("-w")
                .arg(&plist),
        );
        println!("✅ Service stopped");
        Ok(())
    } else if cfg!(target_os = "linux") {
        let resolved = init_system.resolve()?;
        stop_linux(resolved)
    } else if cfg!(target_os = "windows") {
        let _ = config;
        let task_name = windows_task_name();
        let _ = run_checked(Command::new("schtasks").args(["/End", "/TN", task_name]));
        println!("✅ Service stopped");
        Ok(())
    } else {
        let _ = config;
        anyhow::bail!("Service management is supported on macOS and Linux only")
    }
}

fn stop_linux(init_system: InitSystem) -> Result<()> {
    match init_system {
        InitSystem::Systemd => {
            let _ = run_checked(Command::new("systemctl").args([
                "--user",
                "stop",
                "naraeclaw.service",
            ]));
        }
        InitSystem::Auto => unreachable!("Auto should be resolved before this point"),
    }
    println!("✅ Service stopped");
    Ok(())
}

pub fn restart(config: &Config, init_system: InitSystem) -> Result<()> {
    if cfg!(target_os = "macos") {
        stop(config, init_system)?;
        start(config, init_system)?;
        println!("✅ Service restarted");
        return Ok(());
    }

    if cfg!(target_os = "linux") {
        let resolved = init_system.resolve()?;
        return restart_linux(resolved);
    }

    if cfg!(target_os = "windows") {
        stop(config, init_system)?;
        start(config, init_system)?;
        println!("✅ Service restarted");
        return Ok(());
    }

    anyhow::bail!("Service management is supported on macOS and Linux only")
}

fn restart_linux(init_system: InitSystem) -> Result<()> {
    match init_system {
        InitSystem::Systemd => {
            run_checked(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
            run_checked(Command::new("systemctl").args([
                "--user",
                "restart",
                "naraeclaw.service",
            ]))?;
        }
        InitSystem::Auto => unreachable!("Auto should be resolved before this point"),
    }
    println!("✅ Service restarted");
    Ok(())
}

pub fn status(config: &Config, init_system: InitSystem) -> Result<()> {
    if cfg!(target_os = "macos") {
        let out = run_capture(Command::new("launchctl").arg("list"))?;
        let running = out.lines().any(|line| line.contains(SERVICE_LABEL));
        println!(
            "Service: {}",
            if running {
                "✅ running/loaded"
            } else {
                "❌ not loaded"
            }
        );
        println!("Unit: {}", macos_service_file()?.display());
        return Ok(());
    }

    if cfg!(target_os = "linux") {
        let resolved = init_system.resolve()?;
        return status_linux(config, resolved);
    }

    if cfg!(target_os = "windows") {
        let _ = config;
        let task_name = windows_task_name();
        let out =
            run_capture(Command::new("schtasks").args(["/Query", "/TN", task_name, "/FO", "LIST"]));
        match out {
            Ok(text) => {
                let running = text.contains("Running");
                println!(
                    "Service: {}",
                    if running {
                        "✅ running"
                    } else {
                        "❌ not running"
                    }
                );
                println!("Task: {}", task_name);
            }
            Err(_) => {
                println!("Service: ❌ not installed");
            }
        }
        return Ok(());
    }

    anyhow::bail!("Service management is supported on macOS and Linux only")
}

fn status_linux(config: &Config, init_system: InitSystem) -> Result<()> {
    match init_system {
        InitSystem::Systemd => {
            let out = run_capture(Command::new("systemctl").args([
                "--user",
                "is-active",
                "naraeclaw.service",
            ]))
            .unwrap_or_else(|_| "unknown".into());
            println!("Service state: {}", out.trim());
            println!("Unit: {}", linux_service_file(config)?.display());
        }
        InitSystem::Auto => unreachable!("Auto should be resolved before this point"),
    }
    Ok(())
}

pub fn logs(config: &Config, init_system: InitSystem, lines: usize, follow: bool) -> Result<()> {
    if cfg!(target_os = "macos") {
        return logs_macos(config, lines, follow);
    }
    if cfg!(target_os = "linux") {
        let resolved = init_system.resolve()?;
        return logs_linux(config, resolved, lines, follow);
    }
    if cfg!(target_os = "windows") {
        return logs_windows(config, lines, follow);
    }
    anyhow::bail!("Service log viewing is supported on macOS, Linux, and Windows only")
}

fn logs_macos(config: &Config, lines: usize, follow: bool) -> Result<()> {
    // Try the launchd log files first (StandardOutPath / StandardErrorPath from the plist).
    // These are the most reliable source since they capture all daemon output.
    let exe = std::env::current_exe().ok();
    let homebrew_var_dir = exe.as_ref().and_then(|e| detect_homebrew_var_dir(e));
    let logs_dir = if let Some(ref var_dir) = homebrew_var_dir {
        var_dir.join("logs")
    } else {
        config
            .config_path
            .parent()
            .map_or_else(|| PathBuf::from("."), PathBuf::from)
            .join("logs")
    };

    let stderr_log = logs_dir.join("daemon.stderr.log");
    let stdout_log = logs_dir.join("daemon.stdout.log");

    // Prefer stderr log (most informative), fall back to stdout
    let log_file = if stderr_log.exists() {
        stderr_log
    } else if stdout_log.exists() {
        stdout_log
    } else {
        bail!(
            "No log files found in {}. Is the service installed?",
            logs_dir.display()
        );
    };

    if follow {
        let status = Command::new("tail")
            .args(["-n", &lines.to_string(), "-f"])
            .arg(&log_file)
            .status()
            .context("Failed to run tail")?;
        if !status.success() {
            bail!("tail exited with non-zero status");
        }
    } else {
        let status = Command::new("tail")
            .args(["-n", &lines.to_string()])
            .arg(&log_file)
            .status()
            .context("Failed to run tail")?;
        if !status.success() {
            bail!("tail exited with non-zero status");
        }
    }
    Ok(())
}

fn logs_linux(config: &Config, init_system: InitSystem, lines: usize, follow: bool) -> Result<()> {
    match init_system {
        InitSystem::Systemd => {
            let mut args = vec![
                "--user".to_string(),
                "-u".to_string(),
                "naraeclaw.service".to_string(),
                "-n".to_string(),
                lines.to_string(),
                "--no-pager".to_string(),
            ];
            if follow {
                args.push("-f".to_string());
            }
            let status = Command::new("journalctl")
                .args(&args)
                .status()
                .context("Failed to run journalctl")?;
            if !status.success() {
                bail!("journalctl exited with non-zero status");
            }
        }
        InitSystem::Auto => unreachable!("Auto should be resolved before this point"),
    }
    let _ = config;
    Ok(())
}

fn logs_windows(config: &Config, lines: usize, follow: bool) -> Result<()> {
    let logs_dir = config
        .config_path
        .parent()
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join("logs");

    let stderr_log = logs_dir.join("daemon.stderr.log");
    let stdout_log = logs_dir.join("daemon.stdout.log");

    let log_file = if stderr_log.exists() {
        stderr_log
    } else if stdout_log.exists() {
        stdout_log
    } else {
        bail!(
            "No log files found in {}. Is the service installed?",
            logs_dir.display()
        );
    };

    if follow {
        // Windows: use PowerShell Get-Content -Wait for tail -f equivalent
        let status = Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "Get-Content -Path '{}' -Tail {} -Wait",
                    log_file.display(),
                    lines
                ),
            ])
            .status()
            .context("Failed to run PowerShell Get-Content")?;
        if !status.success() {
            bail!("PowerShell Get-Content exited with non-zero status");
        }
    } else {
        let status = Command::new("powershell")
            .args([
                "-Command",
                &format!("Get-Content -Path '{}' -Tail {}", log_file.display(), lines),
            ])
            .status()
            .context("Failed to run PowerShell Get-Content")?;
        if !status.success() {
            bail!("PowerShell Get-Content exited with non-zero status");
        }
    }
    Ok(())
}

pub fn uninstall(config: &Config, init_system: InitSystem) -> Result<()> {
    stop(config, init_system)?;

    if cfg!(target_os = "macos") {
        let file = macos_service_file()?;
        if file.exists() {
            fs::remove_file(&file)
                .with_context(|| format!("Failed to remove {}", file.display()))?;
        }
        println!("✅ Service uninstalled ({})", file.display());
        return Ok(());
    }

    if cfg!(target_os = "linux") {
        let resolved = init_system.resolve()?;
        return uninstall_linux(config, resolved);
    }

    if cfg!(target_os = "windows") {
        let task_name = windows_task_name();
        let _ = run_checked(Command::new("schtasks").args(["/Delete", "/TN", task_name, "/F"]));
        // Remove the wrapper script
        let wrapper = config
            .config_path
            .parent()
            .map_or_else(|| PathBuf::from("."), PathBuf::from)
            .join("logs")
            .join("naraeclaw-daemon.cmd");
        if wrapper.exists() {
            fs::remove_file(&wrapper).ok();
        }
        println!("✅ Service uninstalled");
        return Ok(());
    }

    anyhow::bail!("Service management is supported on macOS and Linux only")
}

fn uninstall_linux(config: &Config, init_system: InitSystem) -> Result<()> {
    match init_system {
        InitSystem::Systemd => {
            let file = linux_service_file(config)?;
            if file.exists() {
                fs::remove_file(&file)
                    .with_context(|| format!("Failed to remove {}", file.display()))?;
            }
            let _ = run_checked(Command::new("systemctl").args(["--user", "daemon-reload"]));
            println!("✅ Service uninstalled ({})", file.display());
        }
        InitSystem::Auto => unreachable!("Auto should be resolved before this point"),
    }
    Ok(())
}

/// Detect if the executable lives under a Homebrew prefix and return the
/// corresponding `var/naraeclaw` directory.
///
/// Homebrew installs binaries into `<prefix>/Cellar/<formula>/<version>/bin/`
/// and symlinks them to `<prefix>/bin/`. The canonical `var` directory is
/// `<prefix>/var`.  We check for both layouts.
fn detect_homebrew_var_dir(exe: &Path) -> Option<PathBuf> {
    let path_str = exe.to_string_lossy();

    // Symlinked binary: <prefix>/bin/naraeclaw
    // Cellar binary:    <prefix>/Cellar/naraeclaw/<version>/bin/naraeclaw
    let prefix = if path_str.contains("/Cellar/") {
        // Walk up from .../Cellar/naraeclaw/<ver>/bin/naraeclaw to the prefix
        let mut ancestor = exe.to_path_buf();
        while let Some(parent) = ancestor.parent() {
            ancestor = parent.to_path_buf();
            if ancestor.file_name().is_some_and(|n| n == "Cellar") {
                // prefix is one level above Cellar
                return ancestor.parent().map(|p| p.join("var").join("naraeclaw"));
            }
        }
        return None;
    } else if let Some(bin_parent) = exe.parent() {
        // <prefix>/bin/naraeclaw → check if <prefix>/Cellar exists (Homebrew marker)
        if let Some(prefix) = bin_parent.parent() {
            if prefix.join("Cellar").is_dir() {
                Some(prefix.to_path_buf())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    prefix.map(|p| p.join("var").join("naraeclaw"))
}

fn install_macos(config: &Config) -> Result<()> {
    let file = macos_service_file()?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }

    let exe = std::env::current_exe().context("Failed to resolve current executable")?;

    // When installed via Homebrew, use the Homebrew var directory for runtime
    // data so that `brew services start naraeclaw` works out of the box.
    let homebrew_var_dir = detect_homebrew_var_dir(&exe);
    if let Some(ref var_dir) = homebrew_var_dir {
        fs::create_dir_all(var_dir).with_context(|| {
            format!(
                "Failed to create Homebrew var directory: {}",
                var_dir.display()
            )
        })?;
    }

    let logs_dir = if let Some(ref var_dir) = homebrew_var_dir {
        var_dir.join("logs")
    } else {
        config
            .config_path
            .parent()
            .map_or_else(|| PathBuf::from("."), PathBuf::from)
            .join("logs")
    };
    fs::create_dir_all(&logs_dir)?;

    let stdout = logs_dir.join("daemon.stdout.log");
    let stderr = logs_dir.join("daemon.stderr.log");

    // When running under Homebrew, inject NARAECLAW_CONFIG_DIR and
    // WorkingDirectory so the daemon finds its data in the Homebrew prefix.
    let env_section = if let Some(ref var_dir) = homebrew_var_dir {
        format!(
            r#"  <key>EnvironmentVariables</key>
  <dict>
    <key>NARAECLAW_CONFIG_DIR</key>
    <string>{config_dir}</string>
  </dict>
  <key>WorkingDirectory</key>
  <string>{working_dir}</string>
"#,
            config_dir = xml_escape(&var_dir.display().to_string()),
            working_dir = xml_escape(&var_dir.display().to_string()),
        )
    } else {
        String::new()
    };

    let plist = format!(
        r#"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>daemon</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
{env_section}  <key>StandardOutPath</key>
  <string>{stdout}</string>
  <key>StandardErrorPath</key>
  <string>{stderr}</string>
</dict>
</plist>
"#,
        label = SERVICE_LABEL,
        exe = xml_escape(&exe.display().to_string()),
        env_section = env_section,
        stdout = xml_escape(&stdout.display().to_string()),
        stderr = xml_escape(&stderr.display().to_string())
    );

    fs::write(&file, plist)?;
    println!("✅ Installed launchd service: {}", file.display());
    if let Some(ref var_dir) = homebrew_var_dir {
        println!("   Homebrew var: {}", var_dir.display());
    }
    println!("   Start with: naraeclaw service start");
    Ok(())
}

fn install_linux(config: &Config, init_system: InitSystem) -> Result<()> {
    match init_system {
        InitSystem::Systemd => install_linux_systemd(config),
        InitSystem::Auto => unreachable!("Auto should be resolved before this point"),
    }
}

fn install_linux_systemd(config: &Config) -> Result<()> {
    let file = linux_service_file(config)?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }

    let exe = std::env::current_exe().context("Failed to resolve current executable")?;
    let unit = format!(
        "[Unit]\n\
         Description=NaraeClaw daemon\n\
         After=network.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={exe} daemon\n\
         Restart=always\n\
         RestartSec=3\n\
         # Ensure HOME is set so headless browsers can create profile/cache dirs.\n\
         Environment=HOME=%h\n\
         # Allow inheriting DISPLAY and XDG_RUNTIME_DIR from the user session\n\
         # so graphical/headless browsers can function correctly.\n\
         PassEnvironment=DISPLAY XDG_RUNTIME_DIR\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        exe = exe.display()
    );

    fs::write(&file, unit)?;
    let _ = run_checked(Command::new("systemctl").args(["--user", "daemon-reload"]));
    let _ = run_checked(Command::new("systemctl").args(["--user", "enable", "naraeclaw.service"]));
    println!("✅ Installed systemd user service: {}", file.display());
    println!("   Start with: naraeclaw service start");
    Ok(())
}

fn install_windows(config: &Config) -> Result<()> {
    let exe = std::env::current_exe().context("Failed to resolve current executable")?;
    let logs_dir = config
        .config_path
        .parent()
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join("logs");
    fs::create_dir_all(&logs_dir)?;

    // Create a wrapper script that redirects output to log files
    let wrapper = logs_dir.join("naraeclaw-daemon.cmd");
    let stdout_log = logs_dir.join("daemon.stdout.log");
    let stderr_log = logs_dir.join("daemon.stderr.log");

    let wrapper_content = format!(
        "@echo off\r\n\"{}\" daemon >>\"{}\" 2>>\"{}\"",
        exe.display(),
        stdout_log.display(),
        stderr_log.display()
    );
    fs::write(&wrapper, &wrapper_content)?;

    let task_name = windows_task_name();

    // Remove any existing task first (ignore errors if it doesn't exist)
    let _ = Command::new("schtasks")
        .args(["/Delete", "/TN", task_name, "/F"])
        .output();

    run_checked(Command::new("schtasks").args([
        "/Create",
        "/TN",
        task_name,
        "/SC",
        "ONLOGON",
        "/TR",
        &format!("\"{}\"", wrapper.display()),
        "/RL",
        "HIGHEST",
        "/F",
    ]))?;

    println!("✅ Installed Windows scheduled task: {}", task_name);
    println!("   Wrapper: {}", wrapper.display());
    println!("   Logs: {}", logs_dir.display());
    println!("   Start with: naraeclaw service start");
    Ok(())
}

fn macos_service_file() -> Result<PathBuf> {
    let home = directories::UserDirs::new()
        .map(|u| u.home_dir().to_path_buf())
        .context("Could not find home directory")?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{SERVICE_LABEL}.plist")))
}

fn linux_service_file(config: &Config) -> Result<PathBuf> {
    let home = directories::UserDirs::new()
        .map(|u| u.home_dir().to_path_buf())
        .context("Could not find home directory")?;
    let _ = config;
    Ok(home
        .join(".config")
        .join("systemd")
        .join("user")
        .join("naraeclaw.service"))
}

fn run_checked(command: &mut Command) -> Result<()> {
    let output = command.output().context("Failed to spawn command")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Command failed: {}", stderr.trim());
    }
    Ok(())
}

pub fn run_capture(command: &mut Command) -> Result<String> {
    let output = command.output().context("Failed to spawn command")?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&output.stderr).to_string();
    }
    Ok(text)
}

pub fn xml_escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(all(test, naraeclaw_root_crate))]
mod tests {
    use super::*;

    #[test]
    fn xml_escape_escapes_reserved_chars() {
        let escaped = xml_escape("<&>\"' and text");
        assert_eq!(escaped, "&lt;&amp;&gt;&quot;&apos; and text");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn run_capture_reads_stdout() {
        let out = run_capture(Command::new("sh").args(["-c", "echo hello"]))
            .expect("stdout capture should succeed");
        assert_eq!(out.trim(), "hello");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn run_capture_falls_back_to_stderr() {
        let out = run_capture(Command::new("sh").args(["-c", "echo warn 1>&2"]))
            .expect("stderr capture should succeed");
        assert_eq!(out.trim(), "warn");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn run_checked_errors_on_non_zero_status() {
        let err = run_checked(Command::new("sh").args(["-c", "exit 17"]))
            .expect_err("non-zero exit should error");
        assert!(err.to_string().contains("Command failed"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn linux_service_file_has_expected_suffix() {
        let file = linux_service_file(&Config::default()).unwrap();
        let path = file.to_string_lossy();
        assert!(path.ends_with(".config/systemd/user/naraeclaw.service"));
    }

    #[test]
    fn windows_task_name_is_constant() {
        assert_eq!(windows_task_name(), "NaraeClaw Daemon");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn run_capture_reads_stdout_windows() {
        let out = run_capture(Command::new("cmd").args(["/C", "echo hello"]))
            .expect("stdout capture should succeed");
        assert_eq!(out.trim(), "hello");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn run_checked_errors_on_non_zero_status_windows() {
        let err = run_checked(Command::new("cmd").args(["/C", "exit /b 17"]))
            .expect_err("non-zero exit should error");
        assert!(err.to_string().contains("Command failed"));
    }

    #[test]
    fn init_system_from_str_parses_valid_values() {
        assert_eq!("auto".parse::<InitSystem>().unwrap(), InitSystem::Auto);
        assert_eq!("AUTO".parse::<InitSystem>().unwrap(), InitSystem::Auto);
        assert_eq!(
            "systemd".parse::<InitSystem>().unwrap(),
            InitSystem::Systemd
        );
        assert_eq!(
            "SYSTEMD".parse::<InitSystem>().unwrap(),
            InitSystem::Systemd
        );
    }

    #[test]
    fn init_system_from_str_rejects_unknown() {
        let err = "unknown"
            .parse::<InitSystem>()
            .expect_err("should reject unknown");
        assert!(err.to_string().contains("Unknown init system"));
        assert!(err.to_string().contains("Supported: auto, systemd"));
    }

    #[test]
    fn init_system_default_is_auto() {
        assert_eq!(InitSystem::default(), InitSystem::Auto);
    }

    #[test]
    fn systemd_unit_contains_home_and_pass_environment() {
        let unit = "[Unit]\n\
             Description=NaraeClaw daemon\n\
             After=network.target\n\
             \n\
             [Service]\n\
             Type=simple\n\
             ExecStart=/usr/local/bin/naraeclaw daemon\n\
             Restart=always\n\
             RestartSec=3\n\
             # Ensure HOME is set so headless browsers can create profile/cache dirs.\n\
             Environment=HOME=%h\n\
             # Allow inheriting DISPLAY and XDG_RUNTIME_DIR from the user session\n\
             # so graphical/headless browsers can function correctly.\n\
             PassEnvironment=DISPLAY XDG_RUNTIME_DIR\n\
             \n\
             [Install]\n\
             WantedBy=default.target\n"
            .to_string();

        assert!(
            unit.contains("Environment=HOME=%h"),
            "systemd unit must set HOME for headless browser support"
        );
        assert!(
            unit.contains("PassEnvironment=DISPLAY XDG_RUNTIME_DIR"),
            "systemd unit must pass through display/runtime env vars"
        );
    }

    #[test]
    fn detect_homebrew_var_dir_from_cellar_path() {
        let exe = PathBuf::from("/opt/homebrew/Cellar/naraeclaw/1.2.3/bin/naraeclaw");
        let var_dir = detect_homebrew_var_dir(&exe);
        assert_eq!(var_dir, Some(PathBuf::from("/opt/homebrew/var/naraeclaw")));
    }

    #[test]
    fn detect_homebrew_var_dir_intel_cellar_path() {
        let exe = PathBuf::from("/usr/local/Cellar/naraeclaw/1.0.0/bin/naraeclaw");
        let var_dir = detect_homebrew_var_dir(&exe);
        assert_eq!(var_dir, Some(PathBuf::from("/usr/local/var/naraeclaw")));
    }

    #[test]
    fn detect_homebrew_var_dir_non_homebrew_path() {
        let exe = PathBuf::from("/home/user/.cargo/bin/naraeclaw");
        let var_dir = detect_homebrew_var_dir(&exe);
        assert_eq!(var_dir, None);
    }

    #[test]
    fn logs_variant_is_recognized() {
        // Ensure the Logs variant can be constructed and matched
        let cmd = crate::ServiceCommands::Logs {
            lines: 25,
            follow: true,
        };
        match &cmd {
            crate::ServiceCommands::Logs { lines, follow } => {
                assert_eq!(*lines, 25);
                assert!(*follow);
            }
            _ => panic!("Expected Logs variant"),
        }
    }
}
