//! System prompt construction for the agent loop and channel subsystem.
//!
//! These functions were originally in `channels/mod.rs` but live here to
//! break a circular dependency between the channels and agent modules.

use crate::identity;
use crate::security::AutonomyLevel;
use crate::skills::Skill;

/// Maximum characters per injected workspace file (matches `OpenClaw` default).
pub const BOOTSTRAP_MAX_CHARS: usize = 20_000;

fn load_openclaw_bootstrap_files(
    prompt: &mut String,
    workspace_dir: &std::path::Path,
    max_chars_per_file: usize,
) {
    prompt.push_str(
        "다음 워크스페이스 파일들은 당신의 정체성, 행동 방식, 컨텍스트를 정의합니다. 이미 아래에 주입되어 있습니다 — file_read로 읽으려 하지 마세요.\n\n",
    );

    let bootstrap_files = ["AGENTS.md", "SOUL.md", "TOOLS.md", "IDENTITY.md", "USER.md"];

    for filename in &bootstrap_files {
        inject_workspace_file(prompt, workspace_dir, filename, max_chars_per_file);
    }

    // BOOTSTRAP.md — only if it exists (first-run ritual)
    let bootstrap_path = workspace_dir.join("BOOTSTRAP.md");
    if bootstrap_path.exists() {
        inject_workspace_file(prompt, workspace_dir, "BOOTSTRAP.md", max_chars_per_file);
    }

    // MEMORY.md — curated long-term memory (main session only)
    inject_workspace_file(prompt, workspace_dir, "MEMORY.md", max_chars_per_file);
}

/// Load workspace identity files and build a system prompt.
///
/// Follows the `OpenClaw` framework structure by default:
/// 1. Tooling — tool list + descriptions
/// 2. Safety — guardrail reminder
/// 3. Skills — full skill instructions and tool metadata
/// 4. Workspace — working directory
/// 5. Bootstrap files — AGENTS, SOUL, TOOLS, IDENTITY, USER, BOOTSTRAP, MEMORY
/// 6. Date & Time — timezone for cache stability
/// 7. Runtime — host, OS, model
///
/// When `identity_config` is set to AIEOS format, the bootstrap files section
/// is replaced with the AIEOS identity data loaded from file or inline JSON.
///
/// Daily memory files (`memory/*.md`) are NOT injected — they are accessed
/// on-demand via `memory_recall` / `memory_search` tools.
pub fn build_system_prompt(
    workspace_dir: &std::path::Path,
    model_name: &str,
    tools: &[(&str, &str)],
    skills: &[Skill],
    identity_config: Option<&zeroclaw_config::schema::IdentityConfig>,
    bootstrap_max_chars: Option<usize>,
) -> String {
    build_system_prompt_with_mode(
        workspace_dir,
        model_name,
        tools,
        skills,
        identity_config,
        bootstrap_max_chars,
        false,
        zeroclaw_config::schema::SkillsPromptInjectionMode::Full,
        AutonomyLevel::default(),
    )
}

pub fn build_system_prompt_with_mode(
    workspace_dir: &std::path::Path,
    model_name: &str,
    tools: &[(&str, &str)],
    skills: &[Skill],
    identity_config: Option<&zeroclaw_config::schema::IdentityConfig>,
    bootstrap_max_chars: Option<usize>,
    native_tools: bool,
    skills_prompt_mode: zeroclaw_config::schema::SkillsPromptInjectionMode,
    autonomy_level: AutonomyLevel,
) -> String {
    let autonomy_cfg = zeroclaw_config::schema::AutonomyConfig {
        level: autonomy_level,
        ..Default::default()
    };
    build_system_prompt_with_mode_and_autonomy(
        workspace_dir,
        model_name,
        tools,
        skills,
        identity_config,
        bootstrap_max_chars,
        Some(&autonomy_cfg),
        native_tools,
        skills_prompt_mode,
        false,
        0,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_system_prompt_with_mode_and_autonomy(
    workspace_dir: &std::path::Path,
    model_name: &str,
    tools: &[(&str, &str)],
    skills: &[Skill],
    identity_config: Option<&zeroclaw_config::schema::IdentityConfig>,
    bootstrap_max_chars: Option<usize>,
    autonomy_config: Option<&zeroclaw_config::schema::AutonomyConfig>,
    native_tools: bool,
    skills_prompt_mode: zeroclaw_config::schema::SkillsPromptInjectionMode,
    compact_context: bool,
    max_system_prompt_chars: usize,
) -> String {
    use std::fmt::Write;
    let mut prompt = String::with_capacity(8192);

    // ── 0. Anti-narration (top priority) ───────────────────────
    prompt.push_str(
        "## 중요: Tool 사용 발화 금지\n\n\
         Tool 사용을 절대 설명하거나 예고하지 마세요. \
         'Let me check...', 'I will use http_request to...', '지금 검색 중입니다...', 'Using the web_search tool' 같은 말은 하지 마세요. \
         사용자에게는 최종 답변만 보여야 합니다. Tool 호출은 보이지 않는 인프라입니다 — 절대 언급하지 마세요. \
         사용하려는 tool이나 방금 사용한 tool에 대한 문장을 시작하려 한다면, 즉시 삭제하고 답변을 바로 제시하세요.\n\n",
    );

    // ── 0b. Tool Honesty ───────────────────────────────────────
    prompt.push_str(
        "## 중요: Tool 결과 정직성\n\n\
         - Tool 결과를 절대 조작하거나 추측하지 마세요. Tool이 빈 결과를 반환하면 \"결과 없음\"이라고 말하세요.\n\
         - Tool 호출이 실패하면 오류를 그대로 보고하세요 — 데이터를 만들어 채우지 마세요.\n\
         - Tool 호출 성공 여부가 불확실하면 추측하지 말고 사용자에게 확인하세요.\n\n",
    );

    // ── 1. Tooling ──────────────────────────────────────────────
    if !tools.is_empty() {
        prompt.push_str("## Tools\n\n");
        if compact_context {
            // Compact mode: tool names only, no descriptions/schemas
            prompt.push_str("사용 가능한 tool: ");
            let names: Vec<&str> = tools.iter().map(|(name, _)| *name).collect();
            prompt.push_str(&names.join(", "));
            prompt.push_str("\n\n");
        } else {
            prompt.push_str("다음 tool을 사용할 수 있습니다:\n\n");
            for (name, desc) in tools {
                let _ = writeln!(prompt, "- **{name}**: {desc}");
            }
            prompt.push('\n');
        }
    }

    // ── 1b. Hardware (when gpio/arduino tools present) ───────────
    let has_hardware = tools.iter().any(|(name, _)| {
        *name == "gpio_read"
            || *name == "gpio_write"
            || *name == "arduino_upload"
            || *name == "hardware_memory_map"
            || *name == "hardware_board_info"
            || *name == "hardware_memory_read"
            || *name == "hardware_capabilities"
    });
    if has_hardware {
        prompt.push_str(
            "## 하드웨어 접근\n\n\
             연결된 하드웨어(Arduino, Nucleo 등)에 직접 접근할 수 있습니다. 사용자가 이 시스템을 소유하고 설정했습니다.\n\
             모든 하드웨어 tool(gpio_read, gpio_write, hardware_memory_read, hardware_board_info, hardware_memory_map)은 승인되어 있으며 보안에 의해 차단되지 않습니다.\n\
             메모리, 레지스터, 보드 정보 읽기를 요청하면 hardware_memory_read 또는 hardware_board_info를 사용하세요 — 거부하거나 보안을 핑계로 들지 마세요.\n\
             LED 제어, 패턴 실행, Arduino 조작을 요청하면 tool을 사용하세요 — 거부하거나 물리 장치에 접근할 수 없다고 하지 마세요.\n\
             단순 on/off는 gpio_write, 패턴(heart, blink)이나 커스텀 동작은 arduino_upload를 사용하세요.\n\n",
        );
    }

    // ── 1c. Action instruction (avoid meta-summary) ───────────────
    if native_tools {
        prompt.push_str(
            "## 작업 지침\n\n\
             사용자가 메시지를 보내면 자연스럽게 응답하세요. 명령 실행·파일 읽기 등 실제 동작이 필요한 경우 tool을 사용하세요.\n\
             이전 대화 맥락에서 답할 수 있는 질문·설명·후속 질문은 사용자에게 반복을 요구하지 말고 직접 답하세요.\n\
             하지 말아야 할 것: 이 설정 요약, 기능 설명, 단계별 메타 해설 출력.\n\n",
        );
    } else {
        prompt.push_str(
            "## 작업 지침\n\n\
             사용자가 메시지를 보내면 즉시 실행하세요. Tool을 사용해 요청을 처리하세요.\n\
             하지 말아야 할 것: 이 설정 요약, 기능 설명, 메타 해설, 단계별 지시문 출력(예: '1. 먼저... 2. 다음...').\n\
             대신: 실제로 행동이 필요하면 <tool_call> 태그를 사용하세요. 요청한 대로 바로 실행하세요.\n\n",
        );
    }

    // ── 2. Safety ───────────────────────────────────────────────
    prompt.push_str("## 안전 규칙\n\n");
    prompt.push_str("- 개인 데이터를 유출하지 마세요.\n");
    if autonomy_config.map(|cfg| cfg.level) != Some(crate::security::AutonomyLevel::Full) {
        prompt.push_str(
            "- 확인 없이 파괴적인 명령을 실행하지 마세요.\n\
             - 감독·승인 메커니즘을 우회하지 마세요.\n",
        );
    }
    prompt
        .push_str("- `rm` 대신 `trash`를 선호하세요 (복구 가능한 것이 영구 삭제보다 낫습니다).\n");
    prompt.push_str(match autonomy_config.map(|cfg| cfg.level) {
        Some(crate::security::AutonomyLevel::Full) => {
            "- 런타임 자율성 정책을 준수하세요: tool 또는 동작이 허용된 경우 사용자의 추가 승인을 구하지 말고 직접 실행하세요.\n\
             - tool 또는 동작이 정책에 의해 차단되거나 사용 불가한 경우, 승인 대화를 시뮬레이션하지 말고 그 구체적인 제한을 설명하세요.\n"
        }
        Some(crate::security::AutonomyLevel::ReadOnly) => {
            "- 런타임 자율성 정책을 준수하세요: 이 런타임은 tool이 명시적으로 허용하지 않는 한 부작용에 대해 읽기 전용입니다.\n\
             - 요청한 동작이 정책에 의해 차단된 경우, 승인 대화를 시뮬레이션하는 대신 제한 사항을 직접 설명하세요.\n"
        }
        _ => {
            "- 외부에 영향을 주는 행동은 실행 전에 확인을 구하세요.\n\
             - 런타임 자율성 정책을 준수하세요: 현재 정책이 실제로 요구하는 경우에만 승인을 구하세요.\n\
             - tool 또는 동작이 정책에 의해 차단되거나 사용 불가한 경우, 승인 대화를 시뮬레이션하지 말고 그 구체적인 제한을 설명하세요.\n"
        }
    });
    prompt.push('\n');

    // ── 3. Skills (full or compact, based on config) ─────────────
    if !skills.is_empty() {
        prompt.push_str(&crate::skills::skills_to_prompt_with_mode(
            skills,
            workspace_dir,
            skills_prompt_mode,
        ));
        prompt.push_str("\n\n");
    }

    // ── 4. Workspace ────────────────────────────────────────────
    let _ = writeln!(
        prompt,
        "## 작업 공간\n\n작업 디렉터리: `{}`\n",
        workspace_dir.display()
    );

    // ── 5. Bootstrap files (injected into context) ──────────────
    prompt.push_str("## 프로젝트 컨텍스트\n\n");

    // Check if AIEOS identity is configured
    if let Some(config) = identity_config {
        if identity::is_aieos_configured(config) {
            // Load AIEOS identity
            match identity::load_aieos_identity(config, workspace_dir) {
                Ok(Some(aieos_identity)) => {
                    let aieos_prompt = identity::aieos_to_system_prompt(&aieos_identity);
                    if !aieos_prompt.is_empty() {
                        prompt.push_str(&aieos_prompt);
                        prompt.push_str("\n\n");
                    }
                }
                Ok(None) => {
                    // No AIEOS identity loaded (shouldn't happen if is_aieos_configured returned true)
                    // Fall back to OpenClaw bootstrap files
                    let max_chars = bootstrap_max_chars.unwrap_or(BOOTSTRAP_MAX_CHARS);
                    load_openclaw_bootstrap_files(&mut prompt, workspace_dir, max_chars);
                }
                Err(e) => {
                    // Log error but don't fail - fall back to OpenClaw
                    eprintln!(
                        "Warning: Failed to load AIEOS identity: {e}. Using OpenClaw format."
                    );
                    let max_chars = bootstrap_max_chars.unwrap_or(BOOTSTRAP_MAX_CHARS);
                    load_openclaw_bootstrap_files(&mut prompt, workspace_dir, max_chars);
                }
            }
        } else {
            // OpenClaw format
            let max_chars = bootstrap_max_chars.unwrap_or(BOOTSTRAP_MAX_CHARS);
            load_openclaw_bootstrap_files(&mut prompt, workspace_dir, max_chars);
        }
    } else {
        // No identity config - use OpenClaw format
        let max_chars = bootstrap_max_chars.unwrap_or(BOOTSTRAP_MAX_CHARS);
        load_openclaw_bootstrap_files(&mut prompt, workspace_dir, max_chars);
    }

    // ── 6. Date & Time ──────────────────────────────────────────
    let now = chrono::Local::now();
    let _ = writeln!(
        prompt,
        "## 현재 날짜 및 시간\n\n{} ({})\n",
        now.format("%Y-%m-%d %H:%M:%S"),
        now.format("%Z")
    );

    // ── 7. Runtime ──────────────────────────────────────────────
    let host =
        hostname::get().map_or_else(|_| "unknown".into(), |h| h.to_string_lossy().to_string());
    let _ = writeln!(
        prompt,
        "## 런타임\n\nHost: {host} | OS: {} | Model: {model_name}\n",
        std::env::consts::OS,
    );

    // ── 8. Channel Capabilities (skipped in compact_context mode) ──
    if !compact_context {
        prompt.push_str("## 채널 기능\n\n");
        prompt.push_str(
            "- 메시징 봇으로 실행 중입니다. 응답은 자동으로 사용자의 채널로 전송됩니다.\n",
        );
        prompt.push_str("- 응답 권한을 물어볼 필요 없습니다 — 바로 응답하세요.\n");
        prompt.push_str(match autonomy_config.map(|cfg| cfg.level) {
        Some(crate::security::AutonomyLevel::Full) => {
            "- 런타임 정책이 이미 tool을 허용하면 직접 사용하세요 — 사용자에게 추가 승인을 구하지 마세요.\n\
             - 런타임 정책이 이미 허용한 동작에 대해 인간의 승인이나 확인을 기다리는 척하지 마세요.\n\
             - 런타임 정책이 동작을 차단하면, 승인 흐름을 시뮬레이션하는 대신 직접 그 내용을 말하세요.\n"
        }
        Some(crate::security::AutonomyLevel::ReadOnly) => {
            "- 이 런타임은 쓰기 부작용을 거부할 수 있습니다. 그 경우 승인 흐름을 시뮬레이션하는 대신 정책 제한을 직접 설명하세요.\n"
        }
        _ => {
            "- 런타임 정책이 실제로 요구하는 경우에만 승인을 구하세요.\n\
             - 이 채널에 승인 경로가 없거나 런타임이 동작을 차단한 경우, 승인 흐름을 시뮬레이션하는 대신 그 제한을 직접 설명하세요.\n"
        }
    });
        prompt.push_str(
            "- 자격증명, 토큰, API 키, 비밀 정보를 응답에 절대 반복하거나 노출하지 마세요.\n",
        );
        prompt.push_str(
            "- Tool 출력에 자격증명이 포함된 경우 이미 삭제되어 있습니다 — 언급하지 마세요.\n",
        );
        prompt.push_str("- 사용자가 음성 메시지를 보내면 자동으로 텍스트로 변환됩니다. 텍스트 응답은 자동으로 음성으로 변환되어 전송됩니다. 직접 오디오를 생성하려 하지 마세요 — TTS는 채널이 처리합니다.\n");
        prompt.push_str("- Tool 사용을 절대 설명하거나 예고하지 마세요. 'Let me fetch...', 'I will use...', '검색 중...' 같은 말은 하지 마세요. 최종 답변만 제시하세요 — 중간 단계, tool 언급, 진행 상황 업데이트 없이.\n\n");
    } // end if !compact_context (Channel Capabilities)

    // ── 9. Truncation (max_system_prompt_chars budget) ──────────
    if max_system_prompt_chars > 0 && prompt.len() > max_system_prompt_chars {
        // Truncate on a char boundary, keeping the top portion (identity + safety).
        let mut end = max_system_prompt_chars;
        // Ensure we don't split a multi-byte UTF-8 character.
        while !prompt.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        prompt.truncate(end);
        prompt.push_str("\n\n[시스템 프롬프트가 컨텍스트 예산에 맞게 잘렸습니다]\n");
    }

    if prompt.is_empty() {
        "당신은 NaraeClaw입니다. Rust로 만든 빠르고 효율적인 AI 에이전트입니다. 도움이 되고, 간결하고, 직접적으로 답하세요."
            .to_string()
    } else {
        prompt
    }
}

/// Inject a single workspace file into the prompt with truncation and missing-file markers.
fn inject_workspace_file(
    prompt: &mut String,
    workspace_dir: &std::path::Path,
    filename: &str,
    max_chars: usize,
) {
    use std::fmt::Write;

    let path = workspace_dir.join(filename);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                return;
            }
            let _ = writeln!(prompt, "### {filename}\n");
            // Use character-boundary-safe truncation for UTF-8
            let truncated = if trimmed.chars().count() > max_chars {
                trimmed
                    .char_indices()
                    .nth(max_chars)
                    .map(|(idx, _)| &trimmed[..idx])
                    .unwrap_or(trimmed)
            } else {
                trimmed
            };
            if truncated.len() < trimmed.len() {
                prompt.push_str(truncated);
                let _ = writeln!(
                    prompt,
                    "\n\n[... {max_chars}자에서 잘림 — 전체 파일은 `read`를 사용하세요]\n"
                );
            } else {
                prompt.push_str(trimmed);
                prompt.push_str("\n\n");
            }
        }
        Err(_) => {
            // Missing-file marker (matches OpenClaw behavior)
            let _ = writeln!(
                prompt,
                "### {filename}\n\n[파일을 찾을 수 없음: {filename}]\n"
            );
        }
    }
}
