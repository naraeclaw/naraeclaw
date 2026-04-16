#![recursion_limit = "256"]
#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::assigning_clones,
    clippy::bool_to_int_with_if,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::cast_possible_wrap,
    clippy::doc_markdown,
    clippy::field_reassign_with_default,
    clippy::float_cmp,
    clippy::implicit_clone,
    clippy::items_after_statements,
    clippy::map_unwrap_or,
    clippy::manual_let_else,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::needless_raw_string_hashes,
    clippy::redundant_closure_for_method_calls,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::struct_field_names,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unused_self,
    clippy::cast_precision_loss,
    clippy::unnecessary_cast,
    clippy::unnecessary_lazy_evaluations,
    clippy::unnecessary_literal_bound,
    clippy::unnecessary_map_or,
    clippy::unnecessary_wraps,
    dead_code,
    unused_variables,
    unused_imports
)]

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use dialoguer::{Password, Select};
use serde::{Deserialize, Serialize};
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt};

fn parse_temperature(s: &str) -> std::result::Result<f64, String> {
    let t: f64 = s.parse().map_err(|e| format!("{e}"))?;
    config::schema::validate_temperature(t)
}

fn print_no_command_help() -> Result<()> {
    println!("명령어를 입력해주세요.");
    println!("`naraeclaw onboard` 로 작업 환경을 초기화할 수 있습니다.");
    println!();

    let mut cmd = Cli::command();
    cmd.print_help()?;
    println!();

    #[cfg(windows)]
    pause_after_no_command_help();

    Ok(())
}

#[cfg(windows)]
fn pause_after_no_command_help() {
    println!();
    print!("Press Enter to exit...");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
}

#[cfg(feature = "agent-runtime")]
mod agent;
#[cfg(feature = "agent-runtime")]
mod approval;
#[cfg(feature = "agent-runtime")]
mod auth;
#[cfg(feature = "agent-runtime")]
mod channels;
#[cfg(feature = "agent-runtime")]
mod cli_input;
mod commands;
mod config;
#[cfg(feature = "agent-runtime")]
mod cost;
#[cfg(feature = "agent-runtime")]
mod cron;
#[cfg(feature = "agent-runtime")]
mod daemon;
#[cfg(feature = "agent-runtime")]
mod doctor;
#[cfg(feature = "gateway")]
mod gateway;
#[cfg(feature = "agent-runtime")]
mod health;
#[cfg(feature = "agent-runtime")]
mod heartbeat;
#[cfg(feature = "agent-runtime")]
mod hooks;
#[cfg(feature = "agent-runtime")]
mod i18n;
#[cfg(feature = "agent-runtime")]
mod identity;
#[cfg(feature = "agent-runtime")]
mod integrations;
mod memory;
#[cfg(feature = "agent-runtime")]
mod migration;
#[cfg(feature = "agent-runtime")]
mod multimodal;
#[cfg(feature = "agent-runtime")]
mod observability;
#[cfg(feature = "agent-runtime")]
mod onboard;
#[cfg(feature = "agent-runtime")]
mod platform;
#[cfg(feature = "plugins-wasm")]
mod plugins;
mod providers;
#[cfg(feature = "agent-runtime")]
mod security;
#[cfg(feature = "agent-runtime")]
mod service;
#[cfg(feature = "agent-runtime")]
mod skillforge;
#[cfg(feature = "agent-runtime")]
mod skills;
#[cfg(feature = "agent-runtime")]
mod sop;
#[cfg(feature = "agent-runtime")]
mod tools;
#[cfg(feature = "agent-runtime")]
mod trust;
#[cfg(feature = "tui-onboarding")]
mod tui;
#[cfg(feature = "agent-runtime")]
mod tunnel;
#[cfg(feature = "agent-runtime")]
mod util;
#[cfg(feature = "agent-runtime")]
mod verifiable_intent;

use config::Config;

// Re-export so binary modules can use crate::<CommandEnum> while keeping a single source of truth.
pub use zeroclaw::{
    ChannelCommands, CronCommands, GatewayCommands, IntegrationCommands, MigrateCommands,
    ServiceCommands, SkillCommands, SopCommands,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum CompletionShell {
    #[value(name = "bash")]
    Bash,
    #[value(name = "fish")]
    Fish,
    #[value(name = "zsh")]
    Zsh,
    #[value(name = "powershell")]
    PowerShell,
    #[value(name = "elvish")]
    Elvish,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum EstopLevelArg {
    #[value(name = "kill-all")]
    KillAll,
    #[value(name = "network-kill")]
    NetworkKill,
    #[value(name = "domain-block")]
    DomainBlock,
    #[value(name = "tool-freeze")]
    ToolFreeze,
}

/// NaraeClaw — 빠르고 가벼운 한국어 AI 에이전트
#[derive(Parser, Debug)]
#[command(name = "naraeclaw")]
#[command(author = "theonlyhennygod")]
#[command(version)]
#[command(about = "빠르고 가벼운 한국어 AI 에이전트", long_about = None)]
struct Cli {
    #[arg(long, global = true)]
    config_dir: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 작업 환경 초기화 및 설정
    Onboard {
        /// 기존 설정을 확인 없이 덮어씁니다
        #[arg(long)]
        force: bool,

        /// 처음부터 재초기화합니다 (기존 설정 백업 후 초기화)
        #[arg(long)]
        reinit: bool,

        /// 채널 설정만 다시 구성합니다 (빠른 복구)
        #[arg(long)]
        channels_only: bool,

        /// Provider 설정에 사용할 API 키
        #[arg(long)]
        api_key: Option<String>,

        /// Provider 이름 (빠른 설정 모드, 기본값: openrouter)
        #[arg(long)]
        provider: Option<String>,
        /// 모델 ID (빠른 설정 모드에서 사용)
        #[arg(long)]
        model: Option<String>,
        /// 메모리 백엔드 (sqlite, lucid, markdown, none) — 빠른 설정 모드, 기본값: sqlite
        #[arg(long)]
        memory: Option<String>,

        /// 대화형 입력 없이 기본값으로 빠르게 설정합니다
        #[arg(long)]
        quick: bool,

        /// ratatui 기반 TUI 온보딩 마법사를 사용합니다
        #[arg(long)]
        tui: bool,
    },

    /// AI 에이전트 루프 시작
    #[command(long_about = "\
AI 에이전트 루프를 시작합니다.

설정된 AI provider와 대화형 세션을 시작합니다.
--message 옵션으로 단일 메시지를 보내고 바로 종료할 수 있습니다.

사용 예:
  naraeclaw agent                              # 대화형 세션
  naraeclaw agent -m \"오늘 로그 요약해줘\"       # 단일 메시지
  naraeclaw agent -p anthropic --model claude-sonnet-4-20250514")]
    Agent {
        /// 단일 메시지 모드 (대화형 세션 없이 메시지 하나만 전송)
        #[arg(short, long)]
        message: Option<String>,

        /// 세션 상태를 저장/불러올 JSON 파일 경로
        #[arg(long)]
        session_state_file: Option<PathBuf>,

        /// 사용할 Provider (openrouter, anthropic, openai, openai-codex)
        #[arg(short, long)]
        provider: Option<String>,

        /// 사용할 모델
        #[arg(long)]
        model: Option<String>,

        /// Temperature (0.0 - 2.0, 기본값: 설정파일의 default_temperature)
        #[arg(short, long, value_parser = parse_temperature)]
        temperature: Option<f64>,
    },

    /// 게이트웨이 서버 시작/관리 (webhook, websocket)
    #[command(long_about = "\
게이트웨이 서버를 관리합니다 (webhook, websocket).

외부 webhook 이벤트와 WebSocket 연결을 받는 HTTP 게이트웨이를
시작, 재시작, 또는 상태 확인합니다.

사용 예:
  naraeclaw gateway start              # 게이트웨이 시작
  naraeclaw gateway restart            # 게이트웨이 재시작
  naraeclaw gateway get-paircode       # 페어링 코드 확인")]
    Gateway {
        #[command(subcommand)]
        gateway_command: Option<zeroclaw::GatewayCommands>,
    },

    /// ACP (Agent Control Protocol) 서버 시작 (stdio)
    #[command(long_about = "\
ACP 서버를 시작합니다 (JSON-RPC 2.0 over stdio).

IDE 및 도구 통합을 위한 JSON-RPC 2.0 서버를 stdin/stdout으로 실행합니다.
세션 관리 및 에이전트 응답 스트리밍을 지원합니다.

메서드: initialize, session/new, session/prompt, session/stop.

사용 예:
  naraeclaw acp                        # ACP 서버 시작
  naraeclaw acp --max-sessions 5       # 최대 동시 세션 수 제한")]
    Acp {
        /// 최대 동시 세션 수 (기본값: 10)
        #[arg(long)]
        max_sessions: Option<usize>,

        /// 세션 비활성 타임아웃 (초, 기본값: 3600)
        #[arg(long)]
        session_timeout: Option<u64>,
    },

    /// 장시간 실행 자율 런타임 시작 (gateway + 채널 + heartbeat + 스케줄러)
    #[command(long_about = "\
장시간 실행 자율 데몬을 시작합니다.

게이트웨이 서버, 설정된 모든 채널(Telegram, Discord, Slack 등),
heartbeat 모니터, cron 스케줄러를 포함한 전체 런타임을 실행합니다.
상시 실행 환경에 권장되는 방식입니다.

'naraeclaw service install' 로 OS 서비스(systemd/launchd)로 등록해
부팅 시 자동 시작할 수 있습니다.

사용 예:
  naraeclaw daemon                   # 설정 기본값 사용
  naraeclaw daemon -p 9090           # 게이트웨이 포트 9090
  naraeclaw daemon --host 127.0.0.1  # localhost만 바인딩")]
    Daemon {
        /// 바인딩할 포트 (0이면 임의 포트, 기본값: 설정파일의 gateway.port)
        #[arg(short, long)]
        port: Option<u16>,

        /// 바인딩할 호스트 (기본값: 설정파일의 gateway.host)
        #[arg(long)]
        host: Option<String>,
    },

    /// OS 서비스 생명주기 관리 (launchd/systemd user service)
    Service {
        /// 초기화 시스템 선택: auto (자동 감지), systemd, openrc
        #[arg(long, default_value = "auto", value_parser = ["auto", "systemd", "openrc"])]
        service_init: String,

        #[command(subcommand)]
        service_command: ServiceCommands,
    },

    /// 데몬/스케줄러/채널 진단 실행
    Doctor {
        #[command(subcommand)]
        doctor_command: Option<DoctorCommands>,
    },

    /// 시스템 상태 확인 (전체 상세 정보)
    Status {
        /// 출력 형식: "exit-code" — 정상이면 0, 비정상이면 1로 종료 (Docker HEALTHCHECK용)
        #[arg(long)]
        format: Option<String>,
    },

    /// 긴급 정지 상태 실행, 확인, 재개
    ///
    /// 사용 예:
    /// - `naraeclaw estop`
    /// - `naraeclaw estop --level network-kill`
    /// - `naraeclaw estop --level domain-block --domain "*.chase.com"`
    /// - `naraeclaw estop --level tool-freeze --tool shell --tool browser`
    /// - `naraeclaw estop status`
    /// - `naraeclaw estop resume --network`
    /// - `naraeclaw estop resume --domain "*.chase.com"`
    /// - `naraeclaw estop resume --tool shell`
    Estop {
        #[command(subcommand)]
        estop_command: Option<EstopSubcommands>,

        /// 긴급 정지 레벨
        #[arg(long, value_enum)]
        level: Option<EstopLevelArg>,

        /// `domain-block` 용 도메인 패턴 (반복 가능)
        #[arg(long = "domain")]
        domains: Vec<String>,

        /// `tool-freeze` 용 tool 이름 (반복 가능)
        #[arg(long = "tool")]
        tools: Vec<String>,
    },

    /// 예약 작업 설정 및 관리
    #[command(long_about = "\
예약 작업을 설정하고 관리합니다.

cron 표현식, RFC 3339 타임스탬프, 지속 시간, 고정 간격으로
반복·일회성·간격 기반 작업을 스케줄링할 수 있습니다.

cron 표현식은 표준 5필드 형식을 사용합니다: 'min hour day month weekday'.
타임존 기본값은 UTC이며 --tz와 IANA 타임존명으로 변경 가능합니다.

사용 예:
  naraeclaw cron list
  naraeclaw cron add '0 9 * * 1-5' '좋은 아침' --tz Asia/Seoul --agent
  naraeclaw cron add '*/30 * * * *' '시스템 상태 확인' --agent
  naraeclaw cron add '*/5 * * * *' 'echo ok'
  naraeclaw cron add-at 2025-01-15T14:00:00Z '리마인더 전송' --agent
  naraeclaw cron add-every 60000 'Ping heartbeat'
  naraeclaw cron once 30m '30분 후 백업 실행' --agent
  naraeclaw cron pause <task-id>
  naraeclaw cron update <task-id> --expression '0 8 * * *' --tz Asia/Seoul")]
    Cron {
        #[command(subcommand)]
        cron_command: CronCommands,
    },

    /// Provider 모델 카탈로그 관리
    Models {
        #[command(subcommand)]
        model_command: ModelCommands,
    },

    /// 지원하는 AI Provider 목록
    Providers,

    /// 채널 관리 (telegram, discord, slack)
    #[command(long_about = "\
통신 채널을 관리합니다.

메시징 플랫폼에 연결하는 채널을 추가, 제거, 목록 확인, 전송,
상태 진단합니다. 지원 채널: telegram, discord, slack, whatsapp, email.

사용 예:
  naraeclaw channel list
  naraeclaw channel doctor
  naraeclaw channel add telegram '{\"bot_token\":\"...\",\"name\":\"my-bot\"}'
  naraeclaw channel remove my-bot
  naraeclaw channel bind-telegram naraeclaw_user
  naraeclaw channel send '알림!' --channel-id telegram --recipient 123456789")]
    Channel {
        #[command(subcommand)]
        channel_command: ChannelCommands,
    },

    /// 50개 이상의 통합 서비스 탐색
    Integrations {
        #[command(subcommand)]
        integration_command: IntegrationCommands,
    },

    /// 스킬 관리 (사용자 정의 기능)
    Skills {
        #[command(subcommand)]
        skill_command: SkillCommands,
    },

    /// SOP (표준 운영 절차) 관리
    Sop {
        #[command(subcommand)]
        sop_command: SopCommands,
    },

    /// 다른 에이전트 런타임에서 데이터 마이그레이션
    Migrate {
        #[command(subcommand)]
        migrate_command: MigrateCommands,
    },

    /// Provider 인증 프로파일 관리
    Auth {
        #[command(subcommand)]
        auth_command: AuthCommands,
    },

    /// 에이전트 메모리 관리 (목록, 조회, 통계, 삭제)
    #[command(long_about = "\
에이전트 메모리 항목을 관리합니다.

에이전트가 저장한 메모리 항목을 목록 확인, 조회, 삭제합니다.
카테고리·세션 필터링, 페이지네이션, 일괄 삭제를 지원합니다.

사용 예:
  naraeclaw memory stats
  naraeclaw memory list
  naraeclaw memory list --category core --limit 10
  naraeclaw memory get <key>
  naraeclaw memory clear --category conversation --yes")]
    Memory {
        #[command(subcommand)]
        memory_command: MemoryCommands,
    },

    /// 설정 관리
    #[command(long_about = "\
NaraeClaw 설정을 관리합니다.

설정 항목을 조회하고 내보냅니다. 'schema' 명령으로 설정 파일의
전체 JSON Schema(모든 키, 타입, 기본값 포함)를 출력합니다.

사용 예:
  naraeclaw config schema              # JSON Schema를 stdout으로 출력
  naraeclaw config schema > schema.json")]
    Config {
        #[command(subcommand)]
        config_command: ConfigCommands,
    },

    /// 업데이트 확인 및 적용
    #[command(long_about = "\
NaraeClaw 업데이트를 확인하고 적용합니다.

기본적으로 최신 릴리즈를 6단계 파이프라인으로 다운로드·설치합니다:
preflight → 다운로드 → 백업 → 검증 → 교체 → 스모크 테스트.
실패 시 자동 롤백됩니다.

사용 예:
  naraeclaw update                      # 최신 버전 다운로드 및 설치
  naraeclaw update --check              # 확인만 (설치 안 함)
  naraeclaw update --force              # 확인 없이 설치
  naraeclaw update --version 0.6.0      # 특정 버전 설치")]
    Update {
        /// 업데이트 확인만 (설치 안 함)
        #[arg(long)]
        check: bool,
        /// 확인 없이 설치
        #[arg(long)]
        force: bool,
        /// 대상 버전 (기본값: 최신)
        #[arg(long)]
        version: Option<String>,
    },

    /// 진단 자가 테스트 실행
    #[command(long_about = "\
NaraeClaw 설치를 검증하는 진단 자가 테스트를 실행합니다.

기본적으로 네트워크 확인(게이트웨이 상태, 메모리 라운드트립)을 포함한
전체 테스트를 실행합니다. --quick으로 네트워크 확인을 건너뛸 수 있습니다.

사용 예:
  naraeclaw self-test             # 전체 테스트
  naraeclaw self-test --quick     # 빠른 확인 (네트워크 제외)")]
    SelfTest {
        /// 빠른 확인만 실행 (네트워크 제외)
        #[arg(long)]
        quick: bool,
    },

    /// 쉘 자동완성 스크립트 생성 (stdout 출력)
    #[command(long_about = "\
naraeclaw의 쉘 자동완성 스크립트를 생성합니다.

스크립트는 stdout으로 출력되므로 직접 source 하거나 파일로 저장하세요.

사용 예:
  source <(naraeclaw completions bash)
  naraeclaw completions zsh > ~/.zfunc/_naraeclaw
  naraeclaw completions fish > ~/.config/fish/completions/naraeclaw.fish")]
    Completions {
        /// 대상 쉘
        #[arg(value_enum)]
        shell: CompletionShell,
    },

    /// 컴패니언 데스크톱 앱 실행 또는 설치
    #[command(long_about = "\
NaraeClaw 컴패니언 데스크톱 앱을 실행합니다.

컴패니언 앱은 CLI와 동일한 게이트웨이에 연결하는 경량 메뉴바/시스템 트레이
애플리케이션입니다. 대시보드 접근, 상태 모니터링, 기기 페어링을 제공합니다.

--install 옵션으로 플랫폼에 맞는 컴패니언 앱을 다운로드·설치합니다.

사용 예:
  naraeclaw desktop              # 컴패니언 앱 실행
  naraeclaw desktop --install    # 다운로드 및 설치")]
    Desktop {
        /// 컴패니언 앱 다운로드 및 설치
        #[arg(long)]
        install: bool,
    },

    /// 점(.) 경로로 설정 속성 조회/변경
    #[command(long_about = "\
설정 속성을 조회, 설정, 초기화합니다.

속성은 점 경로(예: channels.telegram.mention-only)로 지정합니다.
비밀 필드(API 키, 토큰)는 자동으로 마스킹 입력을 사용합니다.
Enum 필드는 값 생략 시 대화형 선택을 제공합니다.

사용 예:
  naraeclaw props list                                       # 모든 속성 목록
  naraeclaw props list --secrets                             # 비밀 필드만
  naraeclaw props list --filter channels.telegram            # 접두사 필터
  naraeclaw props get channels.telegram.mention-only         # 값 조회
  naraeclaw props set channels.telegram.mention-only true    # 값 설정
  naraeclaw props set channels.telegram.bot-token            # 비밀: 마스킹 입력
  naraeclaw props set channels.telegram.stream-mode          # enum: 대화형 선택
  naraeclaw props init channels.telegram                     # 기본값으로 섹션 초기화

속성 경로 탭 자동완성은 'naraeclaw completions <shell>' 에 자동 포함됩니다.")]
    Props {
        #[command(subcommand)]
        props_command: PropsCommands,
    },

    /// Manage WASM plugins
    #[cfg(feature = "plugins-wasm")]
    Plugin {
        #[command(subcommand)]
        plugin_command: PluginCommands,
    },
}

#[derive(Subcommand, Debug)]
enum PropsCommands {
    /// List all config properties with current values
    List {
        /// Filter by path prefix (e.g. "channels.telegram")
        #[arg(short, long)]
        filter: Option<String>,
        /// Show only secret (encrypted) fields
        #[arg(long)]
        secrets: bool,
    },
    /// Get a config property value
    Get {
        /// Property path (e.g. channels.telegram.mention-only)
        path: String,
    },
    /// Set a config property (secret fields auto-prompt for masked input)
    Set {
        /// Property path
        path: String,
        /// New value (omit for secret fields to get masked input)
        value: Option<String>,
        /// Skip interactive prompts — require value on command line, accept raw strings for enums
        #[arg(long)]
        no_interactive: bool,
    },
    /// Initialize unconfigured sections with defaults (enabled=false)
    Init {
        /// Section prefix (e.g. channels.matrix). Omit to init all.
        section: Option<String>,
    },
    /// Print matching property paths for shell completion (hidden)
    #[command(hide = true)]
    Complete {
        /// Partial path to complete
        partial: Option<String>,
    },
}

#[cfg(feature = "plugins-wasm")]
#[derive(Subcommand, Debug)]
enum PluginCommands {
    /// List installed plugins
    List,
    /// Install a plugin from a directory or URL
    Install {
        /// Path to plugin directory or manifest
        source: String,
    },
    /// Remove an installed plugin
    Remove {
        /// Plugin name
        name: String,
    },
    /// Show information about a plugin
    Info {
        /// Plugin name
        name: String,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommands {
    /// Dump the full configuration JSON Schema to stdout
    Schema,
}

#[derive(Subcommand, Debug)]
enum EstopSubcommands {
    /// Print current estop status.
    Status,
    /// Resume from an engaged estop level.
    Resume {
        /// Resume only network kill.
        #[arg(long)]
        network: bool,
        /// Resume one or more blocked domain patterns.
        #[arg(long = "domain")]
        domains: Vec<String>,
        /// Resume one or more frozen tools.
        #[arg(long = "tool")]
        tools: Vec<String>,
        /// OTP code. If omitted and OTP is required, a prompt is shown.
        #[arg(long)]
        otp: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum AuthCommands {
    /// Login with OAuth (OpenAI Codex or Gemini)
    Login {
        /// Provider (`openai-codex` or `gemini`)
        #[arg(long)]
        provider: String,
        /// Profile name (default: default)
        #[arg(long, default_value = "default")]
        profile: String,
        /// Use OAuth device-code flow
        #[arg(long)]
        device_code: bool,
        /// Import an existing auth.json file instead of starting a new login flow.
        /// Currently supports only `openai-codex`; Codex defaults to `~/.codex/auth.json`.
        #[arg(long, value_name = "PATH", conflicts_with = "device_code")]
        import: Option<PathBuf>,
    },
    /// Complete OAuth by pasting redirect URL or auth code
    PasteRedirect {
        /// Provider (`openai-codex`)
        #[arg(long)]
        provider: String,
        /// Profile name (default: default)
        #[arg(long, default_value = "default")]
        profile: String,
        /// Full redirect URL or raw OAuth code
        #[arg(long)]
        input: Option<String>,
    },
    /// Paste setup token / auth token (for Anthropic subscription auth)
    PasteToken {
        /// Provider (`anthropic`)
        #[arg(long)]
        provider: String,
        /// Profile name (default: default)
        #[arg(long, default_value = "default")]
        profile: String,
        /// Token value (if omitted, read interactively)
        #[arg(long)]
        token: Option<String>,
        /// Auth kind override (`authorization` or `api-key`)
        #[arg(long)]
        auth_kind: Option<String>,
    },
    /// Alias for `paste-token` (interactive by default)
    SetupToken {
        /// Provider (`anthropic`)
        #[arg(long)]
        provider: String,
        /// Profile name (default: default)
        #[arg(long, default_value = "default")]
        profile: String,
    },
    /// Refresh OpenAI Codex access token using refresh token
    Refresh {
        /// Provider (`openai-codex`)
        #[arg(long)]
        provider: String,
        /// Profile name or profile id
        #[arg(long)]
        profile: Option<String>,
    },
    /// Remove auth profile
    Logout {
        /// Provider
        #[arg(long)]
        provider: String,
        /// Profile name (default: default)
        #[arg(long, default_value = "default")]
        profile: String,
    },
    /// Set active profile for a provider
    Use {
        /// Provider
        #[arg(long)]
        provider: String,
        /// Profile name or full profile id
        #[arg(long)]
        profile: String,
    },
    /// List auth profiles
    List,
    /// Show auth status with active profile and token expiry info
    Status,
}

#[derive(Subcommand, Debug)]
enum ModelCommands {
    /// Refresh and cache provider models
    Refresh {
        /// Provider name (defaults to configured default provider)
        #[arg(long)]
        provider: Option<String>,

        /// Refresh all providers that support live model discovery
        #[arg(long)]
        all: bool,

        /// Force live refresh and ignore fresh cache
        #[arg(long)]
        force: bool,
    },
    /// List cached models for a provider
    List {
        /// Provider name (defaults to configured default provider)
        #[arg(long)]
        provider: Option<String>,
    },
    /// Set the default model in config
    Set {
        /// Model name to set as default
        model: String,
    },
    /// Show current model configuration and cache status
    Status,
}

#[derive(Subcommand, Debug)]
enum DoctorCommands {
    /// Probe model catalogs across providers and report availability
    Models {
        /// Probe a specific provider only (default: all known providers)
        #[arg(long)]
        provider: Option<String>,

        /// Prefer cached catalogs when available (skip forced live refresh)
        #[arg(long)]
        use_cache: bool,
    },
    /// Query runtime trace events (tool diagnostics and model replies)
    Traces {
        /// Show a specific trace event by id
        #[arg(long)]
        id: Option<String>,
        /// Filter list output by event type
        #[arg(long)]
        event: Option<String>,
        /// Case-insensitive text match across message/payload
        #[arg(long)]
        contains: Option<String>,
        /// Maximum number of events to display
        #[arg(long, default_value = "20")]
        limit: usize,
    },
}

#[derive(Subcommand, Debug)]
enum MemoryCommands {
    /// List memory entries with optional filters
    List {
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        session: Option<String>,
        #[arg(long, default_value = "50")]
        limit: usize,
        #[arg(long, default_value = "0")]
        offset: usize,
    },
    /// Get a specific memory entry by key
    Get { key: String },
    /// Show memory backend statistics and health
    Stats,
    /// Clear memories by category, by key, or clear all
    Clear {
        /// Delete a single entry by key (supports prefix match)
        #[arg(long)]
        key: Option<String>,
        #[arg(long)]
        category: Option<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<()> {
    // Install default crypto provider for Rustls TLS.
    // This prevents the error: "could not automatically determine the process-level CryptoProvider"
    // when both aws-lc-rs and ring features are available (or neither is explicitly selected).
    #[cfg(feature = "agent-runtime")]
    if let Err(e) = rustls::crypto::ring::default_provider().install_default() {
        eprintln!("Warning: Failed to install default crypto provider: {e:?}");
    }

    if std::env::args_os().len() <= 1 {
        return print_no_command_help();
    }

    let cli = Cli::parse();

    if let Some(config_dir) = &cli.config_dir {
        if config_dir.trim().is_empty() {
            bail!("--config-dir cannot be empty");
        }
        // SAFETY: called before Tokio runtime and any threads are spawned.
        // No concurrent readers exist at this point in the process lifecycle.
        // Do NOT move this call into an async context or after runtime::block_on.
        unsafe { std::env::set_var("NARAECLAW_CONFIG_DIR", config_dir) };
    }

    // Completions must remain stdout-only and should not load config or initialize logging.
    // This avoids warnings/log lines corrupting sourced completion scripts.
    if let Commands::Completions { shell } = &cli.command {
        let mut stdout = std::io::stdout().lock();
        write_shell_completion(*shell, &mut stdout)?;
        return Ok(());
    }

    // Initialize logging - respects RUST_LOG env var, defaults to INFO
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Onboard auto-detects the environment: if stdin/stdout are a TTY and no
    // provider flags were given, it runs the full interactive wizard; otherwise
    // it runs the quick (scriptable) setup.  Use --quick to force quick setup,
    // or set ZEROCLAW_INTERACTIVE=1 to force interactive mode when TTY
    // detection fails.  This means `curl … | bash` and
    // `naraeclaw onboard --api-key …` both take the fast path, while a bare
    // `naraeclaw onboard` in a terminal launches the wizard.
    #[cfg(feature = "agent-runtime")]
    if let Commands::Onboard {
        force,
        reinit,
        channels_only,
        api_key,
        provider,
        model,
        memory,
        quick,
        tui: use_tui,
    } = &cli.command
    {
        let force = *force;
        let reinit = *reinit;
        let channels_only = *channels_only;
        let api_key = api_key.clone();
        let provider = provider.clone();
        let model = model.clone();
        let memory = memory.clone();
        let quick = *quick;
        let use_tui = *use_tui;

        if reinit && channels_only {
            bail!("--reinit and --channels-only cannot be used together");
        }
        if channels_only
            && (api_key.is_some() || provider.is_some() || model.is_some() || memory.is_some())
        {
            bail!("--channels-only does not accept --api-key, --provider, --model, or --memory");
        }
        if channels_only && force {
            bail!("--channels-only does not accept --force");
        }
        if quick && channels_only {
            bail!("--quick and --channels-only cannot be used together");
        }

        // Handle --reinit: backup and reset configuration
        if reinit {
            let (zeroclaw_dir, _) =
                crate::config::schema::resolve_runtime_dirs_for_onboarding().await?;

            if zeroclaw_dir.exists() {
                let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S");
                let backup_dir = format!("{}.backup.{}", zeroclaw_dir.display(), timestamp);

                println!("⚠️  Reinitializing NaraeClaw configuration...");
                println!("   Current config directory: {}", zeroclaw_dir.display());
                println!(
                    "   This will back up your existing config to: {}",
                    backup_dir
                );
                println!();
                print!("Continue? [y/N] ");
                std::io::stdout()
                    .flush()
                    .context("Failed to flush stdout")?;

                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if !answer.trim().eq_ignore_ascii_case("y") {
                    println!("Aborted.");
                    return Ok(());
                }
                println!();

                // Rename existing directory as backup
                tokio::fs::rename(&zeroclaw_dir, &backup_dir)
                    .await
                    .with_context(|| {
                        format!("Failed to backup existing config to {}", backup_dir)
                    })?;

                println!("   Backup created successfully.");
                println!("   Starting fresh initialization...\n");
            }
        }

        // Auto-detect: run the interactive wizard when in a TTY with no
        // provider flags, quick setup otherwise (scriptable path).
        let has_provider_flags =
            api_key.is_some() || provider.is_some() || model.is_some() || memory.is_some();
        let is_tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
        let env_interactive = std::env::var("NARAECLAW_INTERACTIVE")
            .or_else(|_| std::env::var("ZEROCLAW_INTERACTIVE"))
            .as_deref()
            == Ok("1");

        // TUI onboarding mode (ratatui-based)
        if use_tui {
            Box::pin(run_tui_if_enabled()).await?;
            return Ok(());
        }

        let wizard_callbacks = build_wizard_callbacks();

        let config = if channels_only {
            Box::pin(onboard::run_channels_repair_wizard(wizard_callbacks)).await
        } else if quick || has_provider_flags {
            Box::pin(onboard::run_quick_setup(
                api_key.as_deref(),
                provider.as_deref(),
                model.as_deref(),
                memory.as_deref(),
                force,
            ))
            .await
        } else if is_tty || env_interactive {
            Box::pin(onboard::run_wizard(force, wizard_callbacks)).await
        } else {
            Box::pin(onboard::run_quick_setup(
                api_key.as_deref(),
                provider.as_deref(),
                model.as_deref(),
                memory.as_deref(),
                force,
            ))
            .await
        }?;

        if config.gateway.require_pairing {
            println!();
            println!("  Pairing is enabled. A one-time pairing code will be");
            println!("  displayed when the gateway starts.");
            println!("  Dashboard: http://127.0.0.1:{}", config.gateway.port);
            println!();
        }

        // Auto-start channels if user said yes during wizard
        if std::env::var("NARAECLAW_AUTOSTART_CHANNELS")
            .or_else(|_| std::env::var("ZEROCLAW_AUTOSTART_CHANNELS"))
            .as_deref()
            == Ok("1")
        {
            Box::pin(channels::start_channels(config)).await?;
        }
        return Ok(());
    }

    // All other commands need config loaded first
    let mut config = Box::pin(Config::load_or_init()).await?;
    config.apply_env_overrides();
    #[cfg(feature = "agent-runtime")]
    observability::runtime_trace::init_from_config(&config.observability, &config.workspace_dir);
    #[cfg(feature = "agent-runtime")]
    if config.security.otp.enabled {
        let config_dir = config
            .config_path
            .parent()
            .context("Config path must have a parent directory")?;
        let store = security::SecretStore::new(config_dir, config.secrets.encrypt);
        let (_validator, enrollment_uri) =
            security::OtpValidator::from_config(&config.security.otp, config_dir, &store)?;
        if let Some(uri) = enrollment_uri {
            println!("Initialized OTP secret for NaraeClaw.");
            println!("Enrollment URI: {uri}");
        }
    }

    #[cfg(not(feature = "agent-runtime"))]
    {
        // Kernel-only mode: minimal CLI agent without channels/tools/gateway
        match cli.command {
            Commands::Agent {
                message,
                provider,
                model,
                temperature,
                ..
            } => {
                let final_temperature = temperature.unwrap_or(config.default_temperature);
                if let Some(p) = &provider {
                    config.default_provider = Some(p.clone());
                }
                if let Some(m) = &model {
                    config.default_model = Some(m.clone());
                }
                config.default_temperature = final_temperature;

                let provider_name = config.default_provider.as_deref().unwrap_or("openai");
                let provider =
                    zeroclaw::providers::create_provider(provider_name, config.api_key.as_deref())?;
                match message {
                    Some(msg) => {
                        let response = provider
                            .simple_chat(
                                &msg,
                                config.default_model.as_deref().unwrap_or("default"),
                                final_temperature,
                            )
                            .await?;
                        println!("{response}");
                    }
                    None => {
                        // Interactive mode
                        let stdin = std::io::stdin();
                        let mut line = String::new();
                        loop {
                            eprint!("> ");
                            line.clear();
                            if stdin.read_line(&mut line)? == 0 {
                                break;
                            }
                            let response = provider
                                .simple_chat(
                                    line.trim(),
                                    config.default_model.as_deref().unwrap_or("default"),
                                    final_temperature,
                                )
                                .await?;
                            println!("{response}");
                        }
                    }
                }
                return Ok(());
            }
            Commands::Completions { shell } => unreachable!(),
            _ => {
                anyhow::bail!(
                    "This command requires the full runtime. Rebuild with default features:\n  cargo build --release"
                );
            }
        }
    }

    #[cfg(feature = "agent-runtime")]
    match cli.command {
        Commands::Onboard { .. } | Commands::Completions { .. } => unreachable!(),

        Commands::Agent {
            message,
            session_state_file,
            provider,
            model,
            temperature,
        } => {
            let final_temperature = temperature.unwrap_or(config.default_temperature);

            zeroclaw_runtime::agent::loop_::register_cli_channel_fn(Box::new(|| {
                Box::new(zeroclaw_channels::cli::CliChannel::new())
            }));

            Box::pin(agent::run(
                config,
                message,
                provider,
                model,
                final_temperature,
                Vec::new(),
                true,
                session_state_file,
                None,
            ))
            .await
            .map(|_| ())
        }

        Commands::Acp {
            max_sessions,
            session_timeout,
        } => {
            let mut acp_config = channels::acp_server::AcpServerConfig::default();
            if let Some(max) = max_sessions {
                acp_config.max_sessions = max;
            }
            if let Some(timeout) = session_timeout {
                acp_config.session_timeout_secs = timeout;
            }
            let server = channels::acp_server::AcpServer::new(config, acp_config);
            server.run().await
        }

        Commands::Gateway { gateway_command } => {
            match gateway_command {
                Some(zeroclaw::GatewayCommands::Restart { port, host }) => {
                    let (port, host) = resolve_gateway_addr(&config, port, host);
                    let addr = format!("{host}:{port}");
                    info!("🔄 Restarting NaraeClaw Gateway on {addr}");

                    // Try to gracefully shutdown existing gateway via admin endpoint
                    match shutdown_gateway(&host, port).await {
                        Ok(()) => {
                            info!("   ✓ Existing gateway on {addr} shut down gracefully");
                            // Poll until the port is free (connection refused) or timeout
                            let deadline =
                                tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
                            loop {
                                match tokio::net::TcpStream::connect(&addr).await {
                                    Err(_) => break, // port is free
                                    Ok(_) if tokio::time::Instant::now() >= deadline => {
                                        warn!(
                                            "   Timed out waiting for port {port} to be released"
                                        );
                                        break;
                                    }
                                    Ok(_) => {
                                        tokio::time::sleep(tokio::time::Duration::from_millis(50))
                                            .await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            info!("   No existing gateway to shut down: {e}");
                        }
                    }

                    log_gateway_start(&host, port);
                    Box::pin(run_gateway_if_enabled(&host, port, config, None)).await
                }
                Some(zeroclaw::GatewayCommands::GetPaircode { new }) => {
                    let port = config.gateway.port;
                    let host = &config.gateway.host;

                    // Fetch live pairing code from running gateway
                    // If --new is specified, generate a fresh pairing code
                    match fetch_paircode(host, port, new).await {
                        Ok(Some(code)) => {
                            println!("🔐 Gateway pairing is enabled.");
                            println!();
                            println!("  ┌──────────────┐");
                            println!("  │  {code}  │");
                            println!("  └──────────────┘");
                            println!();
                            println!("  Use this one-time code to pair a new device:");
                            println!("    POST /pair with header X-Pairing-Code: {code}");
                        }
                        Ok(None) => {
                            if config.gateway.require_pairing {
                                println!(
                                    "🔐 Gateway pairing is enabled, but no active pairing code available."
                                );
                                println!(
                                    "   The gateway may already be paired, or the code has been used."
                                );
                                println!("   Restart the gateway to generate a new pairing code.");
                            } else {
                                println!("⚠️  Gateway pairing is disabled in config.");
                                println!(
                                    "   All requests will be accepted without authentication."
                                );
                                println!(
                                    "   To enable pairing, set [gateway] require_pairing = true"
                                );
                            }
                        }
                        Err(e) => {
                            println!(
                                "❌ Failed to fetch pairing code from gateway at {host}:{port}"
                            );
                            println!("   Error: {e}");
                            println!();
                            println!("   Is the gateway running? Start it with:");
                            println!("     naraeclaw gateway start");
                        }
                    }
                    Ok(())
                }
                Some(zeroclaw::GatewayCommands::Start { port, host }) => {
                    let (port, host) = resolve_gateway_addr(&config, port, host);
                    log_gateway_start(&host, port);
                    Box::pin(run_gateway_if_enabled(&host, port, config, None)).await
                }
                None => {
                    let port = config.gateway.port;
                    let host = config.gateway.host.clone();
                    log_gateway_start(&host, port);
                    Box::pin(run_gateway_if_enabled(&host, port, config, None)).await
                }
            }
        }

        Commands::Daemon { port, host } => {
            if let Ok(exe) = std::env::current_exe() {
                let exe_str = exe.to_string_lossy();
                if exe_str.contains(".cargo/bin") || exe_str.contains("/home/") {
                    tracing::warn!(
                        "Daemon running from user home directory: {}. \
                         Consider installing to /usr/local/bin for system-wide service.",
                        exe_str
                    );
                }
            }
            let port = port.unwrap_or(config.gateway.port);
            let host = host.unwrap_or_else(|| config.gateway.host.clone());
            if port == 0 {
                info!("🧠 Starting NaraeClaw Daemon on {host} (random port)");
            } else {
                info!("🧠 Starting NaraeClaw Daemon on {host}:{port}");
            }
            // Wire CLI channel for interactive mode
            #[cfg(feature = "agent-runtime")]
            zeroclaw_runtime::agent::loop_::register_cli_channel_fn(Box::new(|| {
                Box::new(zeroclaw_channels::cli::CliChannel::new())
            }));

            // Wire cron delivery to the channels orchestrator
            #[cfg(feature = "agent-runtime")]
            zeroclaw_runtime::cron::scheduler::register_delivery_fn(Box::new(
                |config, channel, target, output| {
                    Box::pin(async move {
                        zeroclaw_channels::orchestrator::deliver_announcement(
                            &config, &channel, &target, &output,
                        )
                        .await
                    })
                },
            ));

            let subsystems = daemon::DaemonSubsystems {
                #[cfg(feature = "gateway")]
                gateway_start: Some(Box::new(|host, port, config, tx| {
                    Box::pin(async move {
                        Box::pin(zeroclaw_gateway::run_gateway(&host, port, config, tx)).await
                    })
                })),
                #[cfg(not(feature = "gateway"))]
                gateway_start: None,
                channels_start: Some(Box::new(|config| {
                    Box::pin(async move {
                        Box::pin(zeroclaw_channels::orchestrator::start_channels(config)).await
                    })
                })),
                mqtt_start: Some(Box::new(|mqtt_config| {
                    Box::pin(async move {
                        use std::sync::{Arc, Mutex};
                        use zeroclaw_config::schema::SopConfig;
                        use zeroclaw_memory::NoneMemory;
                        use zeroclaw_runtime::sop::{SopAuditLogger, SopEngine};

                        let engine = Arc::new(Mutex::new(SopEngine::new(SopConfig::default())));
                        let audit = Arc::new(SopAuditLogger::new(Arc::new(NoneMemory)));
                        zeroclaw_channels::orchestrator::mqtt::run_mqtt_sop_listener(
                            &mqtt_config,
                            engine,
                            audit,
                        )
                        .await
                    })
                })),
            };
            Box::pin(daemon::run(config, host, port, subsystems)).await
        }

        Commands::Status { format } => {
            if format.as_deref() == Some("exit-code") {
                // Lightweight health probe for Docker HEALTHCHECK
                let port = config.gateway.port;
                let host = if config.gateway.host == "[::]" || config.gateway.host == "0.0.0.0" {
                    "127.0.0.1"
                } else {
                    &config.gateway.host
                };
                let url = format!("http://{}:{}/health", host, port);
                match reqwest::Client::new()
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(5))
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => {
                        std::process::exit(0);
                    }
                    _ => {
                        std::process::exit(1);
                    }
                }
            }
            println!("🦀 NaraeClaw Status");
            println!();
            println!("Version:     {}", env!("CARGO_PKG_VERSION"));
            println!("Workspace:   {}", config.workspace_dir.display());
            println!("Config:      {}", config.config_path.display());
            println!();
            println!(
                "🤖 Provider:      {}",
                config.default_provider.as_deref().unwrap_or("openrouter")
            );
            println!(
                "   Model:         {}",
                config.default_model.as_deref().unwrap_or("(default)")
            );
            println!("📊 Observability:  {}", config.observability.backend);
            println!(
                "🧾 Trace storage:  {} ({})",
                config.observability.runtime_trace_mode, config.observability.runtime_trace_path
            );
            println!("🛡️  Autonomy:      {:?}", config.autonomy.level);
            println!("⚙️  Runtime:       {}", config.runtime.kind);
            if service::is_running() {
                println!("🟢 Service:       running");
            } else {
                println!("🔴 Service:       stopped");
            }
            let effective_memory_backend = memory::effective_memory_backend_name(
                &config.memory.backend,
                Some(&config.storage.provider.config),
            );
            println!(
                "💓 Heartbeat:      {}",
                if config.heartbeat.enabled {
                    format!("every {}min", config.heartbeat.interval_minutes)
                } else {
                    "disabled".into()
                }
            );
            println!(
                "🧠 Memory:         {} (auto-save: {})",
                effective_memory_backend,
                if config.memory.auto_save { "on" } else { "off" }
            );

            println!();
            println!("Security:");
            println!("  Workspace only:    {}", config.autonomy.workspace_only);
            println!(
                "  Allowed roots:     {}",
                if config.autonomy.allowed_roots.is_empty() {
                    "(none)".to_string()
                } else {
                    config.autonomy.allowed_roots.join(", ")
                }
            );
            println!(
                "  Allowed commands:  {}",
                config.autonomy.allowed_commands.join(", ")
            );
            println!(
                "  Max actions/hour:  {}",
                config.autonomy.max_actions_per_hour
            );
            println!(
                "  Cost tracking:     {}",
                if config.cost.enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!("  Max cost/day:      ${:.2}", config.cost.daily_limit_usd);
            println!("  Max cost/month:    ${:.2}", config.cost.monthly_limit_usd);
            if config.cost.enabled {
                match cost::CostTracker::new(config.cost.clone(), &config.workspace_dir) {
                    Ok(tracker) => match tracker.get_summary() {
                        Ok(summary) => {
                            println!(
                                "  Spent today:       ${:.4} / ${:.2}",
                                summary.daily_cost_usd, config.cost.daily_limit_usd
                            );
                            println!(
                                "  Spent this month:  ${:.4} / ${:.2}",
                                summary.monthly_cost_usd, config.cost.monthly_limit_usd
                            );
                        }
                        Err(e) => {
                            eprintln!("  ⚠ Could not load cost usage: {e}");
                        }
                    },
                    Err(e) => {
                        eprintln!("  ⚠ Could not init cost tracker: {e}");
                    }
                }
            }
            println!("  OTP enabled:       {}", config.security.otp.enabled);
            println!("  E-stop enabled:    {}", config.security.estop.enabled);
            println!();
            println!("Channels:");
            println!("  CLI:      ✅ always");
            for (channel, configured) in config.channels_config.channels() {
                println!(
                    "  {:9} {}",
                    channel.name(),
                    if configured {
                        "✅ configured"
                    } else {
                        "❌ not configured"
                    }
                );
            }

            Ok(())
        }

        Commands::Estop {
            estop_command,
            level,
            domains,
            tools,
        } => handle_estop_command(&config, estop_command, level, domains, tools),

        Commands::Cron { cron_command } => cron::handle_command(cron_command, &config),

        Commands::Models { model_command } => match model_command {
            ModelCommands::Refresh {
                provider,
                all,
                force,
            } => {
                if all {
                    if provider.is_some() {
                        bail!("`models refresh --all` cannot be combined with --provider");
                    }
                    onboard::run_models_refresh_all(&config, force).await
                } else {
                    onboard::run_models_refresh(&config, provider.as_deref(), force).await
                }
            }
            ModelCommands::List { provider } => {
                onboard::run_models_list(&config, provider.as_deref()).await
            }
            ModelCommands::Set { model } => {
                Box::pin(onboard::run_models_set(&config, &model)).await
            }
            ModelCommands::Status => onboard::run_models_status(&config).await,
        },

        Commands::Providers => {
            let providers = providers::list_providers();
            let current = config
                .default_provider
                .as_deref()
                .unwrap_or("openrouter")
                .trim()
                .to_ascii_lowercase();
            println!("Supported providers ({} total):\n", providers.len());
            println!("  ID (use in config)  DESCRIPTION");
            println!("  ─────────────────── ───────────");
            for p in &providers {
                let is_active = p.name.eq_ignore_ascii_case(&current)
                    || p.aliases
                        .iter()
                        .any(|alias| alias.eq_ignore_ascii_case(&current));
                let marker = if is_active { " (active)" } else { "" };
                let local_tag = if p.local { " [local]" } else { "" };
                let aliases = if p.aliases.is_empty() {
                    String::new()
                } else {
                    format!("  (aliases: {})", p.aliases.join(", "))
                };
                println!(
                    "  {:<19} {}{}{}{}",
                    p.name, p.display_name, local_tag, marker, aliases
                );
            }
            println!("\n  custom:<URL>   Any OpenAI-compatible endpoint");
            println!("  anthropic-custom:<URL>  Any Anthropic-compatible endpoint");
            Ok(())
        }

        Commands::Service {
            service_command,
            service_init,
        } => {
            let init_system = service_init.parse()?;
            service::handle_command(&service_command, &config, init_system)
        }

        Commands::Doctor { doctor_command } => match doctor_command {
            Some(DoctorCommands::Models {
                provider,
                use_cache,
            }) => doctor::run_models(&config, provider.as_deref(), use_cache).await,
            Some(DoctorCommands::Traces {
                id,
                event,
                contains,
                limit,
            }) => doctor::run_traces(
                &config,
                id.as_deref(),
                event.as_deref(),
                contains.as_deref(),
                limit,
            ),
            None => doctor::run(&config),
        },

        Commands::Channel { channel_command } => match channel_command {
            ChannelCommands::Start => Box::pin(channels::start_channels(config)).await,
            ChannelCommands::Doctor => Box::pin(channels::doctor_channels(config)).await,
            other => Box::pin(channels::handle_command(other, &config)).await,
        },

        Commands::Integrations {
            integration_command,
        } => integrations::handle_command(integration_command, &config),

        Commands::Skills { skill_command } => skills::handle_command(skill_command, &config),

        Commands::Sop { sop_command } => sop::handle_command(sop_command, &config),

        Commands::Migrate { migrate_command } => {
            migration::handle_command(migrate_command, &config).await
        }

        Commands::Memory { memory_command } => {
            memory::cli::handle_command(memory_command, &config).await
        }

        Commands::Auth { auth_command } => handle_auth_command(auth_command, &config).await,

        Commands::Desktop {
            install: do_install,
        } => {
            let download_url = "https://www.naraeclaw.ai/download";

            if do_install {
                println!("Download the NaraeClaw companion app:");
                println!();
                #[cfg(target_os = "macos")]
                {
                    println!("  macOS:  {download_url}");
                    println!();
                    println!("Or install via Homebrew (coming soon):");
                    println!("  brew install --cask naraeclaw");
                }
                #[cfg(target_os = "linux")]
                {
                    println!("  Linux:  {download_url}");
                    println!();
                    println!("  Download the .deb or .AppImage for your architecture.");
                }
                #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                {
                    println!("  {download_url}");
                }
                println!();

                // On macOS, open the download page in the browser
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open").arg(download_url).spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(download_url)
                        .spawn();
                }
                return Ok(());
            }

            // Locate the companion app
            let desktop_bin = {
                let mut found = None;

                // 1. macOS: check /Applications/NaraeClaw.app
                #[cfg(target_os = "macos")]
                {
                    let app_paths = [
                        PathBuf::from("/Applications/NaraeClaw.app/Contents/MacOS/NaraeClaw"),
                        PathBuf::from(std::env::var("HOME").unwrap_or_default())
                            .join("Applications/NaraeClaw.app/Contents/MacOS/NaraeClaw"),
                    ];
                    for app in &app_paths {
                        if app.is_file() {
                            found = Some(app.clone());
                            break;
                        }
                    }
                }

                // 2. Same directory as the current executable
                if found.is_none() {
                    if let Ok(exe) = std::env::current_exe() {
                        let sibling = exe.with_file_name("naraeclaw-desktop");
                        if sibling.is_file() {
                            found = Some(sibling);
                        }
                    }
                }

                // 3. ~/.cargo/bin/naraeclaw-desktop or ~/.local/bin/naraeclaw-desktop
                if found.is_none() {
                    if let Some(home) = std::env::var_os("HOME") {
                        let home = PathBuf::from(home);
                        for dir in &[".cargo/bin", ".local/bin"] {
                            let candidate = home.join(dir).join("naraeclaw-desktop");
                            if candidate.is_file() {
                                found = Some(candidate);
                                break;
                            }
                        }
                    }
                }

                // 4. Fallback to PATH lookup
                if found.is_none() {
                    if let Ok(path) = which::which("naraeclaw-desktop") {
                        found = Some(path);
                    }
                }

                found
            };

            match desktop_bin {
                Some(bin) => {
                    println!("Launching NaraeClaw companion app...");
                    let _child = std::process::Command::new(&bin)
                        .spawn()
                        .with_context(|| format!("Failed to launch {}", bin.display()))?;
                    Ok(())
                }
                None => {
                    println!("NaraeClaw companion app is not installed.");
                    println!();
                    println!("  Download it at: {download_url}");
                    println!("  Or run: naraeclaw desktop --install");
                    println!();
                    println!("The companion app is a lightweight menu bar app that");
                    println!("connects to the same gateway as the CLI.");
                    std::process::exit(1);
                }
            }
        }

        Commands::Update {
            check,
            force: _force,
            version,
        } => {
            if check {
                let info = commands::update::check(version.as_deref()).await?;
                if info.is_newer {
                    println!(
                        "Update available: v{} -> v{}",
                        info.current_version, info.latest_version
                    );
                } else {
                    println!("Already up to date (v{}).", info.current_version);
                }
                Ok(())
            } else {
                commands::update::run(version.as_deref()).await
            }
        }

        Commands::SelfTest { quick } => {
            let results = if quick {
                commands::self_test::run_quick(&config).await?
            } else {
                commands::self_test::run_full(&config).await?
            };
            commands::self_test::print_results(&results);
            let failed = results.iter().filter(|r| !r.passed).count();
            if failed > 0 {
                std::process::exit(1);
            }
            Ok(())
        }

        Commands::Config { config_command } => match config_command {
            ConfigCommands::Schema => {
                let schema = schemars::schema_for!(config::Config);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&schema).expect("failed to serialize JSON Schema")
                );
                Ok(())
            }
        },

        Commands::Props { props_command } => match props_command {
            PropsCommands::List { filter, secrets } => {
                let entries = config.prop_fields();
                let mut current_category = "";
                for entry in &entries {
                    if secrets && !entry.is_secret {
                        continue;
                    }
                    if let Some(ref f) = filter {
                        if !entry.name.starts_with(f.as_str()) {
                            continue;
                        }
                    }
                    if entry.category != current_category {
                        if !current_category.is_empty() {
                            println!();
                        }
                        println!("{}:", entry.category);
                        current_category = entry.category;
                    }
                    let lock = if entry.is_secret { " \u{1f512}" } else { "" };
                    println!(
                        "  {:<45} = {:<20} ({}){lock}",
                        entry.name, entry.display_value, entry.type_hint
                    );
                }
                Ok(())
            }
            PropsCommands::Get { path } => {
                if Config::prop_is_secret(&path) {
                    let entries = config.prop_fields();
                    let is_set = entries
                        .iter()
                        .find(|e| e.name == path)
                        .map(|e| e.display_value != "<unset>")
                        .unwrap_or(false);
                    if is_set {
                        println!("{path} is set (encrypted secret \u{2014} value not displayed)");
                    } else {
                        println!("{path} is not set (encrypted secret)");
                    }
                } else {
                    match config.get_prop(&path) {
                        Ok(value) => println!("{value}"),
                        Err(e) => anyhow::bail!("{e}"),
                    }
                }
                Ok(())
            }
            PropsCommands::Set {
                path,
                value,
                no_interactive,
            } => {
                if no_interactive {
                    // Scripted mode: require value on CLI, no prompts
                    let val = value.ok_or_else(|| {
                        anyhow::anyhow!(
                            "Value required in --no-interactive mode. Usage: naraeclaw props set --no-interactive {path} <value>"
                        )
                    })?;
                    config.set_prop(&path, &val)?;
                } else if Config::prop_is_secret(&path) {
                    if value.is_some() {
                        eprintln!(
                            "  \u{26a0} {path} is an encrypted secret \u{2014} using masked input."
                        );
                    }
                    let secret_value = dialoguer::Password::new()
                        .with_prompt(format!("Enter value for {path}"))
                        .interact()?;
                    let secret_value = secret_value.trim().to_string();
                    if secret_value.is_empty() {
                        anyhow::bail!("Value cannot be empty.");
                    }
                    config.set_prop(&path, &secret_value)?;
                } else if let Some(val) = value {
                    config.set_prop(&path, &val)?;
                } else {
                    // Enum fields get interactive selection; everything else needs a value
                    let variants = config
                        .prop_fields()
                        .into_iter()
                        .find(|f| f.name == path)
                        .and_then(|info| {
                            let get_variants = info.enum_variants?;
                            let variants = get_variants();
                            let current_index = variants
                                .iter()
                                .position(|v| v == &info.display_value)
                                .unwrap_or(0);
                            Some((variants, current_index))
                        });
                    if let Some((variants, current_index)) = variants {
                        let selected = Select::new()
                            .with_prompt(format!("Select value for {path}"))
                            .items(&variants)
                            .default(current_index)
                            .interact()?;
                        config.set_prop(&path, &variants[selected])?;
                    } else {
                        anyhow::bail!("Value required. Usage: naraeclaw props set {path} <value>");
                    }
                }
                config.save().await?;
                println!("{path} updated.");
                Ok(())
            }
            PropsCommands::Init { section } => {
                let initialized = config.init_defaults(section.as_deref());
                if initialized.is_empty() {
                    println!("All sections already configured.");
                } else {
                    println!(
                        "Initialized {} section(s) with defaults:",
                        initialized.len()
                    );
                    for name in &initialized {
                        println!("  {name}");
                    }
                    config.save().await?;
                    println!("\nRun `naraeclaw props list` to review, then set required fields.");
                }
                Ok(())
            }
            PropsCommands::Complete { partial } => {
                let prefix = partial.as_deref().unwrap_or("");
                for entry in config.prop_fields() {
                    if entry.name.starts_with(prefix) {
                        println!("{}", entry.name);
                    }
                }
                Ok(())
            }
        },

        #[cfg(feature = "plugins-wasm")]
        Commands::Plugin { plugin_command } => match plugin_command {
            PluginCommands::List => {
                let host = zeroclaw::plugins::host::PluginHost::new(&config.workspace_dir)?;
                let plugins = host.list_plugins();
                if plugins.is_empty() {
                    println!("No plugins installed.");
                } else {
                    println!("Installed plugins:");
                    for p in &plugins {
                        println!(
                            "  {} v{} — {}",
                            p.name,
                            p.version,
                            p.description.as_deref().unwrap_or("(no description)")
                        );
                    }
                }
                Ok(())
            }
            PluginCommands::Install { source } => {
                let mut host = zeroclaw::plugins::host::PluginHost::new(&config.workspace_dir)?;
                host.install(&source)?;
                println!("Plugin installed from {source}");
                Ok(())
            }
            PluginCommands::Remove { name } => {
                let mut host = zeroclaw::plugins::host::PluginHost::new(&config.workspace_dir)?;
                host.remove(&name)?;
                println!("Plugin '{name}' removed.");
                Ok(())
            }
            PluginCommands::Info { name } => {
                let host = zeroclaw::plugins::host::PluginHost::new(&config.workspace_dir)?;
                match host.get_plugin(&name) {
                    Some(info) => {
                        println!("Plugin: {} v{}", info.name, info.version);
                        if let Some(desc) = &info.description {
                            println!("Description: {desc}");
                        }
                        println!("Capabilities: {:?}", info.capabilities);
                        println!("Permissions: {:?}", info.permissions);
                        println!("WASM: {}", info.wasm_path.display());
                    }
                    None => println!("Plugin '{name}' not found."),
                }
                Ok(())
            }
        },
    }
}

/// Build wizard callbacks for optional onboarding integrations.
#[cfg(feature = "agent-runtime")]
fn build_wizard_callbacks() -> onboard::WizardCallbacks {
    onboard::WizardCallbacks {
        #[cfg(feature = "channel-nostr")]
        nostr_validate_key: Some(Box::new(|key: &str| {
            let keys = nostr_sdk::Keys::parse(key)
                .map_err(|e| anyhow::anyhow!("invalid nostr key: {e}"))?;
            Ok(keys.public_key().to_hex())
        })),

        whatsapp_web_available: cfg!(feature = "whatsapp-web"),
    }
}

#[cfg(feature = "agent-runtime")]
fn handle_estop_command(
    config: &Config,
    estop_command: Option<EstopSubcommands>,
    level: Option<EstopLevelArg>,
    domains: Vec<String>,
    tools: Vec<String>,
) -> Result<()> {
    if !config.security.estop.enabled {
        bail!("Emergency stop is disabled. Enable [security.estop].enabled = true in config.toml");
    }

    let config_dir = config
        .config_path
        .parent()
        .context("Config path must have a parent directory")?;
    let mut manager = security::EstopManager::load(&config.security.estop, config_dir)?;

    match estop_command {
        Some(EstopSubcommands::Status) => {
            print_estop_status(&manager.status());
            Ok(())
        }
        Some(EstopSubcommands::Resume {
            network,
            domains,
            tools,
            otp,
        }) => {
            let selector = build_resume_selector(network, domains, tools)?;
            let mut otp_code = otp;
            let otp_validator = if config.security.estop.require_otp_to_resume {
                if !config.security.otp.enabled {
                    bail!(
                        "security.estop.require_otp_to_resume=true but security.otp.enabled=false"
                    );
                }
                if otp_code.is_none() {
                    let entered = Password::new()
                        .with_prompt("Enter OTP code")
                        .allow_empty_password(false)
                        .interact()?;
                    otp_code = Some(entered);
                }

                let store = security::SecretStore::new(config_dir, config.secrets.encrypt);
                let (validator, enrollment_uri) =
                    security::OtpValidator::from_config(&config.security.otp, config_dir, &store)?;
                if let Some(uri) = enrollment_uri {
                    println!("Initialized OTP secret for NaraeClaw.");
                    println!("Enrollment URI: {uri}");
                }
                Some(validator)
            } else {
                None
            };

            manager.resume(selector, otp_code.as_deref(), otp_validator.as_ref())?;
            println!("Estop resume completed.");
            print_estop_status(&manager.status());
            Ok(())
        }
        None => {
            let engage_level = build_engage_level(level, domains, tools)?;
            manager.engage(engage_level)?;
            println!("Estop engaged.");
            print_estop_status(&manager.status());
            Ok(())
        }
    }
}

#[cfg(feature = "agent-runtime")]
fn build_engage_level(
    level: Option<EstopLevelArg>,
    domains: Vec<String>,
    tools: Vec<String>,
) -> Result<security::EstopLevel> {
    let requested = level.unwrap_or(EstopLevelArg::KillAll);
    match requested {
        EstopLevelArg::KillAll => {
            if !domains.is_empty() || !tools.is_empty() {
                bail!("--domain/--tool are only valid with --level domain-block/tool-freeze");
            }
            Ok(security::EstopLevel::KillAll)
        }
        EstopLevelArg::NetworkKill => {
            if !domains.is_empty() || !tools.is_empty() {
                bail!("--domain/--tool are not valid with --level network-kill");
            }
            Ok(security::EstopLevel::NetworkKill)
        }
        EstopLevelArg::DomainBlock => {
            if domains.is_empty() {
                bail!("--level domain-block requires at least one --domain");
            }
            if !tools.is_empty() {
                bail!("--tool is not valid with --level domain-block");
            }
            Ok(security::EstopLevel::DomainBlock(domains))
        }
        EstopLevelArg::ToolFreeze => {
            if tools.is_empty() {
                bail!("--level tool-freeze requires at least one --tool");
            }
            if !domains.is_empty() {
                bail!("--domain is not valid with --level tool-freeze");
            }
            Ok(security::EstopLevel::ToolFreeze(tools))
        }
    }
}

#[cfg(feature = "agent-runtime")]
fn build_resume_selector(
    network: bool,
    domains: Vec<String>,
    tools: Vec<String>,
) -> Result<security::ResumeSelector> {
    let selected =
        usize::from(network) + usize::from(!domains.is_empty()) + usize::from(!tools.is_empty());
    if selected > 1 {
        bail!("Use only one of --network, --domain, or --tool for estop resume");
    }
    if network {
        return Ok(security::ResumeSelector::Network);
    }
    if !domains.is_empty() {
        return Ok(security::ResumeSelector::Domains(domains));
    }
    if !tools.is_empty() {
        return Ok(security::ResumeSelector::Tools(tools));
    }
    Ok(security::ResumeSelector::KillAll)
}

#[cfg(feature = "agent-runtime")]
fn print_estop_status(state: &security::EstopState) {
    println!("Estop status:");
    println!(
        "  engaged:        {}",
        if state.is_engaged() { "yes" } else { "no" }
    );
    println!(
        "  kill_all:       {}",
        if state.kill_all { "active" } else { "inactive" }
    );
    println!(
        "  network_kill:   {}",
        if state.network_kill {
            "active"
        } else {
            "inactive"
        }
    );
    if state.blocked_domains.is_empty() {
        println!("  domain_blocks:  (none)");
    } else {
        println!("  domain_blocks:  {}", state.blocked_domains.join(", "));
    }
    if state.frozen_tools.is_empty() {
        println!("  tool_freeze:    (none)");
    } else {
        println!("  tool_freeze:    {}", state.frozen_tools.join(", "));
    }
    if let Some(updated_at) = &state.updated_at {
        println!("  updated_at:     {updated_at}");
    }
}

fn write_shell_completion<W: Write>(shell: CompletionShell, writer: &mut W) -> Result<()> {
    use clap_complete::generate;
    use clap_complete::shells;

    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();

    match shell {
        CompletionShell::Bash => {
            generate(shells::Bash, &mut cmd, bin_name.clone(), writer);
            // Wrap clap's _naraeclaw to inject dynamic props path completion
            writeln!(
                writer,
                r#"
# Dynamic completion for naraeclaw props get/set paths
if type _naraeclaw &>/dev/null; then
    _naraeclaw_clap_orig() {{ _naraeclaw "$@"; }}
    _naraeclaw() {{
        local cur="${{COMP_WORDS[COMP_CWORD]}}"
        if [[ "${{COMP_WORDS[*]}}" =~ "props "(get|set)" " ]]; then
            COMPREPLY=($(compgen -W "$(naraeclaw props complete "$cur" 2>/dev/null)" -- "$cur"))
            return
        fi
        _naraeclaw_clap_orig "$@"
    }}
fi"#
            )?;
        }
        CompletionShell::Fish => {
            generate(shells::Fish, &mut cmd, bin_name.clone(), writer);
            writeln!(
                writer,
                r#"
# Dynamic completion for naraeclaw props get/set paths
complete -c naraeclaw -n '__fish_seen_subcommand_from props; and __fish_seen_subcommand_from get set' \
    -a '(naraeclaw props complete (commandline -ct) 2>/dev/null)' -f"#
            )?;
        }
        CompletionShell::Zsh => {
            generate(shells::Zsh, &mut cmd, bin_name.clone(), writer);
            // Wrap clap's _naraeclaw to inject dynamic props path completion
            writeln!(
                writer,
                r#"
# Dynamic completion for naraeclaw props get/set paths
if (( $+functions[_naraeclaw] )); then
    functions[_naraeclaw_clap_orig]=$functions[_naraeclaw]
    _naraeclaw() {{
        if [[ "${{words[*]}}" == *"props "(get|set)* ]] && (( CURRENT > 3 )); then
            local -a props
            props=(${{(f)"$(naraeclaw props complete "$words[CURRENT]" 2>/dev/null)"}})
            compadd -a props
            return
        fi
        _naraeclaw_clap_orig "$@"
    }}
fi"#
            )?;
        }
        CompletionShell::PowerShell => {
            generate(shells::PowerShell, &mut cmd, bin_name.clone(), writer);
        }
        CompletionShell::Elvish => generate(shells::Elvish, &mut cmd, bin_name, writer),
    }

    writer.flush()?;
    Ok(())
}

// ─── Gateway helper functions ───────────────────────────────────────────────

/// Resolve gateway host and port from CLI args or config.
fn resolve_gateway_addr(config: &Config, port: Option<u16>, host: Option<String>) -> (u16, String) {
    let port = port.unwrap_or(config.gateway.port);
    let host = host.unwrap_or_else(|| config.gateway.host.clone());
    (port, host)
}

/// Log gateway startup message.
fn log_gateway_start(host: &str, port: u16) {
    if port == 0 {
        info!("🚀 Starting NaraeClaw Gateway on {host} (random port)");
    } else {
        info!("🚀 Starting NaraeClaw Gateway on {host}:{port}");
    }
}

/// Gracefully shutdown a running gateway via the admin endpoint.
#[cfg(feature = "agent-runtime")]
async fn shutdown_gateway(host: &str, port: u16) -> Result<()> {
    let url = format!("http://{host}:{port}/admin/shutdown");
    let client = reqwest::Client::new();

    match client
        .post(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => Ok(()),
        Ok(response) => Err(anyhow::anyhow!(
            "Gateway responded with status: {}",
            response.status()
        )),
        Err(e) => Err(anyhow::anyhow!("Failed to connect to gateway: {e}")),
    }
}

/// Fetch the current pairing code from a running gateway.
/// If `new` is true, generates a fresh pairing code via POST request.
#[cfg(feature = "agent-runtime")]
async fn fetch_paircode(host: &str, port: u16, new: bool) -> Result<Option<String>> {
    let client = reqwest::Client::new();

    let response = if new {
        // Generate a new pairing code via POST
        let url = format!("http://{host}:{port}/admin/paircode/new");
        client
            .post(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
    } else {
        // Get existing pairing code via GET
        let url = format!("http://{host}:{port}/admin/paircode");
        client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
    };

    let response = response.map_err(|e| anyhow::anyhow!("Failed to connect to gateway: {e}"))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Gateway responded with status: {}",
            response.status()
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse response: {e}"))?;

    if json.get("success").and_then(|v| v.as_bool()) != Some(true) {
        return Ok(None);
    }

    Ok(json
        .get("pairing_code")
        .and_then(|v| v.as_str())
        .map(String::from))
}

// ─── Generic Pending OAuth Login ────────────────────────────────────────────

/// Generic pending OAuth login state, shared across providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingOAuthLogin {
    provider: String,
    profile: String,
    code_verifier: String,
    state: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingOAuthLoginFile {
    #[serde(default)]
    provider: Option<String>,
    profile: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_verifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encrypted_code_verifier: Option<String>,
    state: String,
    created_at: String,
}

#[cfg(feature = "agent-runtime")]
fn pending_oauth_login_path(config: &Config, provider: &str) -> std::path::PathBuf {
    let filename = format!("auth-{}-pending.json", provider);
    auth::state_dir_from_config(config).join(filename)
}

#[cfg(feature = "agent-runtime")]
fn pending_oauth_secret_store(config: &Config) -> security::secrets::SecretStore {
    security::secrets::SecretStore::new(
        &auth::state_dir_from_config(config),
        config.secrets.encrypt,
    )
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

#[cfg(feature = "agent-runtime")]
fn save_pending_oauth_login(config: &Config, pending: &PendingOAuthLogin) -> Result<()> {
    let path = pending_oauth_login_path(config, &pending.provider);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let secret_store = pending_oauth_secret_store(config);
    let encrypted_code_verifier = secret_store.encrypt(&pending.code_verifier)?;
    let persisted = PendingOAuthLoginFile {
        provider: Some(pending.provider.clone()),
        profile: pending.profile.clone(),
        code_verifier: None,
        encrypted_code_verifier: Some(encrypted_code_verifier),
        state: pending.state.clone(),
        created_at: pending.created_at.clone(),
    };
    let tmp = path.with_extension(format!(
        "tmp.{}.{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let json = serde_json::to_vec_pretty(&persisted)?;
    std::fs::write(&tmp, json)?;
    set_owner_only_permissions(&tmp)?;
    std::fs::rename(tmp, &path)?;
    set_owner_only_permissions(&path)?;
    Ok(())
}

#[cfg(feature = "agent-runtime")]
fn load_pending_oauth_login(config: &Config, provider: &str) -> Result<Option<PendingOAuthLogin>> {
    let path = pending_oauth_login_path(config, provider);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path)?;
    if bytes.is_empty() {
        return Ok(None);
    }
    let persisted: PendingOAuthLoginFile = serde_json::from_slice(&bytes)?;
    let secret_store = pending_oauth_secret_store(config);
    let code_verifier = if let Some(encrypted) = persisted.encrypted_code_verifier {
        secret_store.decrypt(&encrypted)?
    } else if let Some(plaintext) = persisted.code_verifier {
        plaintext
    } else {
        bail!("Pending {} login is missing code verifier", provider);
    };
    Ok(Some(PendingOAuthLogin {
        provider: persisted.provider.unwrap_or_else(|| provider.to_string()),
        profile: persisted.profile,
        code_verifier,
        state: persisted.state,
        created_at: persisted.created_at,
    }))
}

#[cfg(feature = "agent-runtime")]
fn clear_pending_oauth_login(config: &Config, provider: &str) {
    let path = pending_oauth_login_path(config, provider);
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(&path) {
        let _ = file.set_len(0);
        let _ = file.sync_all();
    }
    let _ = std::fs::remove_file(path);
}

#[cfg(feature = "agent-runtime")]
fn read_auth_input(prompt: &str) -> Result<String> {
    let input = Password::new()
        .with_prompt(prompt)
        .allow_empty_password(false)
        .interact()?;
    Ok(input.trim().to_string())
}

#[cfg(feature = "agent-runtime")]
fn read_plain_input(prompt: &str) -> Result<String> {
    let input: String = cli_input::Input::new()
        .with_prompt(prompt)
        .interact_text()?;
    Ok(input.trim().to_string())
}

#[cfg(feature = "agent-runtime")]
fn extract_openai_account_id_for_profile(access_token: &str) -> Option<String> {
    let account_id = auth::openai_oauth::extract_account_id_from_jwt(access_token);
    if account_id.is_none() {
        warn!(
            "Could not extract OpenAI account id from OAuth access token; \
             requests may fail until re-authentication."
        );
    }
    account_id
}

#[cfg(feature = "agent-runtime")]
async fn import_openai_codex_auth_profile(
    auth_service: &auth::AuthService,
    profile: &str,
    import_path: &std::path::Path,
) -> Result<()> {
    #[derive(Deserialize)]
    struct CodexAuthTokens {
        access_token: String,
        #[serde(default)]
        refresh_token: Option<String>,
        #[serde(default)]
        id_token: Option<String>,
        #[serde(default)]
        account_id: Option<String>,
    }

    #[derive(Deserialize)]
    struct CodexAuthFile {
        tokens: CodexAuthTokens,
    }

    let raw = std::fs::read_to_string(import_path)
        .with_context(|| format!("Failed to read import file {}", import_path.display()))?;
    let imported: CodexAuthFile = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse import file {}", import_path.display()))?;
    let expires_at = auth::openai_oauth::extract_expiry_from_jwt(&imported.tokens.access_token);

    let token_set = auth::profiles::TokenSet {
        access_token: imported.tokens.access_token,
        refresh_token: imported.tokens.refresh_token,
        id_token: imported.tokens.id_token,
        expires_at,
        token_type: Some("Bearer".to_string()),
        scope: None,
    };

    let account_id = imported
        .tokens
        .account_id
        .or_else(|| extract_openai_account_id_for_profile(&token_set.access_token));

    auth_service
        .store_openai_tokens(profile, token_set, account_id, true)
        .await?;

    Ok(())
}

#[cfg(feature = "agent-runtime")]
fn format_expiry(profile: &auth::profiles::AuthProfile) -> String {
    match profile
        .token_set
        .as_ref()
        .and_then(|token_set| token_set.expires_at)
    {
        Some(ts) => {
            let now = chrono::Utc::now();
            if ts <= now {
                format!("expired at {}", ts.to_rfc3339())
            } else {
                let mins = (ts - now).num_minutes();
                format!("expires in {mins}m ({})", ts.to_rfc3339())
            }
        }
        None => "n/a".to_string(),
    }
}

#[allow(clippy::too_many_lines)]
#[cfg(feature = "agent-runtime")]
async fn handle_auth_command(auth_command: AuthCommands, config: &Config) -> Result<()> {
    let auth_service = auth::AuthService::from_config(config);

    match auth_command {
        AuthCommands::Login {
            provider,
            profile,
            device_code,
            import,
        } => {
            let provider = auth::normalize_provider(&provider)?;
            if import.is_some() && provider != "openai-codex" {
                bail!("`auth login --import` currently supports only --provider openai-codex");
            }
            let client = reqwest::Client::new();

            match provider.as_str() {
                "gemini" => {
                    // Gemini OAuth flow
                    if device_code {
                        match auth::gemini_oauth::start_device_code_flow(&client).await {
                            Ok(device) => {
                                println!("Google/Gemini device-code login started.");
                                println!("Visit: {}", device.verification_uri);
                                println!("Code:  {}", device.user_code);
                                if let Some(uri_complete) = &device.verification_uri_complete {
                                    println!("Fast link: {uri_complete}");
                                }

                                let token_set =
                                    auth::gemini_oauth::poll_device_code_tokens(&client, &device)
                                        .await?;
                                let account_id = token_set.id_token.as_deref().and_then(
                                    auth::gemini_oauth::extract_account_email_from_id_token,
                                );

                                auth_service
                                    .store_gemini_tokens(&profile, token_set, account_id, true)
                                    .await?;

                                println!("Saved profile {profile}");
                                println!("Active profile for gemini: {profile}");
                                return Ok(());
                            }
                            Err(e) => {
                                println!(
                                    "Device-code flow unavailable: {e}. Falling back to browser flow."
                                );
                            }
                        }
                    }

                    let pkce = auth::gemini_oauth::generate_pkce_state();
                    let authorize_url = auth::gemini_oauth::build_authorize_url(&pkce)?;

                    // Save pending login for paste-redirect fallback
                    let pending = PendingOAuthLogin {
                        provider: "gemini".to_string(),
                        profile: profile.clone(),
                        code_verifier: pkce.code_verifier.clone(),
                        state: pkce.state.clone(),
                        created_at: chrono::Utc::now().to_rfc3339(),
                    };
                    save_pending_oauth_login(config, &pending)?;

                    println!("Open this URL in your browser and authorize access:");
                    println!("{authorize_url}");
                    println!();

                    let code = match auth::gemini_oauth::receive_loopback_code(
                        &pkce.state,
                        std::time::Duration::from_secs(180),
                    )
                    .await
                    {
                        Ok(code) => {
                            clear_pending_oauth_login(config, "gemini");
                            code
                        }
                        Err(e) => {
                            println!("Callback capture failed: {e}");
                            println!(
                                "Run `zeroclaw auth paste-redirect --provider gemini --profile {profile}`"
                            );
                            return Ok(());
                        }
                    };

                    let token_set =
                        auth::gemini_oauth::exchange_code_for_tokens(&client, &code, &pkce).await?;
                    let account_id = token_set
                        .id_token
                        .as_deref()
                        .and_then(auth::gemini_oauth::extract_account_email_from_id_token);

                    auth_service
                        .store_gemini_tokens(&profile, token_set, account_id, true)
                        .await?;

                    println!("Saved profile {profile}");
                    println!("Active profile for gemini: {profile}");
                    Ok(())
                }
                "openai-codex" => {
                    if let Some(import_path) = import.as_deref() {
                        import_openai_codex_auth_profile(&auth_service, &profile, import_path)
                            .await?;
                        println!("Imported auth profile from {}", import_path.display());
                        println!("Active profile for openai-codex: {profile}");
                        return Ok(());
                    }

                    // OpenAI Codex OAuth flow
                    if device_code {
                        match auth::openai_oauth::start_device_code_flow(&client).await {
                            Ok(device) => {
                                println!("OpenAI device-code login started.");
                                println!("Visit: {}", device.verification_uri);
                                println!("Code:  {}", device.user_code);
                                if let Some(uri_complete) = &device.verification_uri_complete {
                                    println!("Fast link: {uri_complete}");
                                }
                                if let Some(message) = &device.message {
                                    println!("{message}");
                                }

                                let token_set =
                                    auth::openai_oauth::poll_device_code_tokens(&client, &device)
                                        .await?;
                                let account_id =
                                    extract_openai_account_id_for_profile(&token_set.access_token);

                                auth_service
                                    .store_openai_tokens(&profile, token_set, account_id, true)
                                    .await?;
                                clear_pending_oauth_login(config, "openai");

                                println!("Saved profile {profile}");
                                println!("Active profile for openai-codex: {profile}");
                                return Ok(());
                            }
                            Err(e) => {
                                println!(
                                    "Device-code flow unavailable: {e}. Falling back to browser/paste flow."
                                );
                            }
                        }
                    }

                    let pkce = auth::openai_oauth::generate_pkce_state();
                    let pending = PendingOAuthLogin {
                        provider: "openai".to_string(),
                        profile: profile.clone(),
                        code_verifier: pkce.code_verifier.clone(),
                        state: pkce.state.clone(),
                        created_at: chrono::Utc::now().to_rfc3339(),
                    };
                    save_pending_oauth_login(config, &pending)?;

                    let authorize_url = auth::openai_oauth::build_authorize_url(&pkce);
                    println!("Open this URL in your browser and authorize access:");
                    println!("{authorize_url}");
                    println!();
                    println!("Waiting for callback at http://localhost:1455/auth/callback ...");

                    let code = match auth::openai_oauth::receive_loopback_code(
                        &pkce.state,
                        std::time::Duration::from_secs(180),
                    )
                    .await
                    {
                        Ok(code) => code,
                        Err(e) => {
                            println!("Callback capture failed: {e}");
                            println!(
                                "Run `zeroclaw auth paste-redirect --provider openai-codex --profile {profile}`"
                            );
                            return Ok(());
                        }
                    };

                    let token_set =
                        auth::openai_oauth::exchange_code_for_tokens(&client, &code, &pkce).await?;
                    let account_id = extract_openai_account_id_for_profile(&token_set.access_token);

                    auth_service
                        .store_openai_tokens(&profile, token_set, account_id, true)
                        .await?;
                    clear_pending_oauth_login(config, "openai");

                    println!("Saved profile {profile}");
                    println!("Active profile for openai-codex: {profile}");
                    Ok(())
                }
                _ => {
                    bail!(
                        "`auth login` supports --provider openai-codex or gemini, got: {provider}"
                    );
                }
            }
        }

        AuthCommands::PasteRedirect {
            provider,
            profile,
            input,
        } => {
            let provider = auth::normalize_provider(&provider)?;

            match provider.as_str() {
                "openai-codex" => {
                    let pending = load_pending_oauth_login(config, "openai")?.ok_or_else(|| {
                        anyhow::anyhow!(
                            "No pending OpenAI login found. Run `zeroclaw auth login --provider openai-codex` first."
                        )
                    })?;

                    if pending.profile != profile {
                        bail!(
                            "Pending login profile mismatch: pending={}, requested={}",
                            pending.profile,
                            profile
                        );
                    }

                    let redirect_input = match input {
                        Some(value) => value,
                        None => read_plain_input("Paste redirect URL or OAuth code")?,
                    };

                    let code = auth::openai_oauth::parse_code_from_redirect(
                        &redirect_input,
                        Some(&pending.state),
                    )?;

                    let pkce = auth::openai_oauth::PkceState {
                        code_verifier: pending.code_verifier.clone(),
                        code_challenge: String::new(),
                        state: pending.state.clone(),
                    };

                    let client = reqwest::Client::new();
                    let token_set =
                        auth::openai_oauth::exchange_code_for_tokens(&client, &code, &pkce).await?;
                    let account_id = extract_openai_account_id_for_profile(&token_set.access_token);

                    auth_service
                        .store_openai_tokens(&profile, token_set, account_id, true)
                        .await?;
                    clear_pending_oauth_login(config, "openai");

                    println!("Saved profile {profile}");
                    println!("Active profile for openai-codex: {profile}");
                }
                "gemini" => {
                    let pending = load_pending_oauth_login(config, "gemini")?.ok_or_else(|| {
                        anyhow::anyhow!(
                            "No pending Gemini login found. Run `zeroclaw auth login --provider gemini` first."
                        )
                    })?;

                    if pending.profile != profile {
                        bail!(
                            "Pending login profile mismatch: pending={}, requested={}",
                            pending.profile,
                            profile
                        );
                    }

                    let redirect_input = match input {
                        Some(value) => value,
                        None => read_plain_input("Paste redirect URL or OAuth code")?,
                    };

                    let code = auth::gemini_oauth::parse_code_from_redirect(
                        &redirect_input,
                        Some(&pending.state),
                    )?;

                    let pkce = auth::gemini_oauth::PkceState {
                        code_verifier: pending.code_verifier.clone(),
                        code_challenge: String::new(),
                        state: pending.state.clone(),
                    };

                    let client = reqwest::Client::new();
                    let token_set =
                        auth::gemini_oauth::exchange_code_for_tokens(&client, &code, &pkce).await?;
                    let account_id = token_set
                        .id_token
                        .as_deref()
                        .and_then(auth::gemini_oauth::extract_account_email_from_id_token);

                    auth_service
                        .store_gemini_tokens(&profile, token_set, account_id, true)
                        .await?;
                    clear_pending_oauth_login(config, "gemini");

                    println!("Saved profile {profile}");
                    println!("Active profile for gemini: {profile}");
                }
                _ => {
                    bail!("`auth paste-redirect` supports --provider openai-codex or gemini");
                }
            }
            Ok(())
        }

        AuthCommands::PasteToken {
            provider,
            profile,
            token,
            auth_kind,
        } => {
            let provider = auth::normalize_provider(&provider)?;
            let token = match token {
                Some(token) => token.trim().to_string(),
                None => read_auth_input("Paste token")?,
            };
            if token.is_empty() {
                bail!("Token cannot be empty");
            }

            let kind = auth::anthropic_token::detect_auth_kind(&token, auth_kind.as_deref());
            let mut metadata = std::collections::HashMap::new();
            metadata.insert(
                "auth_kind".to_string(),
                kind.as_metadata_value().to_string(),
            );

            auth_service
                .store_provider_token(&provider, &profile, &token, metadata, true)
                .await?;
            println!("Saved profile {profile}");
            println!("Active profile for {provider}: {profile}");
            Ok(())
        }

        AuthCommands::SetupToken { provider, profile } => {
            let provider = auth::normalize_provider(&provider)?;
            let token = read_auth_input("Paste token")?;
            if token.is_empty() {
                bail!("Token cannot be empty");
            }

            let kind = auth::anthropic_token::detect_auth_kind(&token, Some("authorization"));
            let mut metadata = std::collections::HashMap::new();
            metadata.insert(
                "auth_kind".to_string(),
                kind.as_metadata_value().to_string(),
            );

            auth_service
                .store_provider_token(&provider, &profile, &token, metadata, true)
                .await?;
            println!("Saved profile {profile}");
            println!("Active profile for {provider}: {profile}");
            Ok(())
        }

        AuthCommands::Refresh { provider, profile } => {
            let provider = auth::normalize_provider(&provider)?;

            match provider.as_str() {
                "openai-codex" => {
                    match auth_service
                        .get_valid_openai_access_token(profile.as_deref())
                        .await?
                    {
                        Some(_) => {
                            println!("OpenAI Codex token is valid (refresh completed if needed).");
                            Ok(())
                        }
                        None => {
                            bail!(
                                "No OpenAI Codex auth profile found. Run `zeroclaw auth login --provider openai-codex`."
                            )
                        }
                    }
                }
                "gemini" => {
                    match auth_service
                        .get_valid_gemini_access_token(profile.as_deref())
                        .await?
                    {
                        Some(_) => {
                            let profile_name = profile.as_deref().unwrap_or("default");
                            println!("✓ Gemini token refreshed successfully");
                            println!("  Profile: gemini:{}", profile_name);
                            Ok(())
                        }
                        None => {
                            bail!(
                                "No Gemini auth profile found. Run `zeroclaw auth login --provider gemini`."
                            )
                        }
                    }
                }
                _ => bail!("`auth refresh` supports --provider openai-codex or gemini"),
            }
        }

        AuthCommands::Logout { provider, profile } => {
            let provider = auth::normalize_provider(&provider)?;
            let removed = auth_service.remove_profile(&provider, &profile).await?;
            if removed {
                println!("Removed auth profile {provider}:{profile}");
            } else {
                println!("Auth profile not found: {provider}:{profile}");
            }
            Ok(())
        }

        AuthCommands::Use { provider, profile } => {
            let provider = auth::normalize_provider(&provider)?;
            auth_service.set_active_profile(&provider, &profile).await?;
            println!("Active profile for {provider}: {profile}");
            Ok(())
        }

        AuthCommands::List => {
            let data = auth_service.load_profiles().await?;
            if data.profiles.is_empty() {
                println!("No auth profiles configured.");
                return Ok(());
            }

            for (id, profile) in &data.profiles {
                let active = data
                    .active_profiles
                    .get(&profile.provider)
                    .is_some_and(|active_id| active_id == id);
                let marker = if active { "*" } else { " " };
                println!("{marker} {id}");
            }

            Ok(())
        }

        AuthCommands::Status => {
            let data = auth_service.load_profiles().await?;
            if data.profiles.is_empty() {
                println!("No auth profiles configured.");
                return Ok(());
            }

            for (id, profile) in &data.profiles {
                let active = data
                    .active_profiles
                    .get(&profile.provider)
                    .is_some_and(|active_id| active_id == id);
                let marker = if active { "*" } else { " " };
                println!(
                    "{} {} kind={:?} account={} expires={}",
                    marker,
                    id,
                    profile.kind,
                    crate::security::redact(profile.account_id.as_deref().unwrap_or("unknown")),
                    format_expiry(profile)
                );
            }

            println!();
            println!("Active profiles:");
            for (provider, profile_id) in &data.active_profiles {
                println!("  {provider}: {profile_id}");
            }

            Ok(())
        }
    }
}

#[cfg(feature = "gateway")]
async fn run_gateway_if_enabled(
    host: &str,
    port: u16,
    config: zeroclaw::config::Config,
    tx: Option<tokio::sync::broadcast::Sender<serde_json::Value>>,
) -> anyhow::Result<()> {
    Box::pin(gateway::run_gateway(host, port, config, tx)).await
}

#[cfg(not(feature = "gateway"))]
#[allow(clippy::unused_async)]
async fn run_gateway_if_enabled(
    _host: &str,
    _port: u16,
    _config: zeroclaw::config::Config,
    _tx: Option<tokio::sync::broadcast::Sender<serde_json::Value>>,
) -> anyhow::Result<()> {
    anyhow::bail!("Gateway feature is not enabled. Rebuild with --features gateway")
}

#[cfg(feature = "tui-onboarding")]
async fn run_tui_if_enabled() -> anyhow::Result<()> {
    Box::pin(tui::run_tui_onboarding()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn cli_definition_has_no_flag_conflicts() {
        Cli::command().debug_assert();
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn onboard_help_includes_model_flag() {
        let cmd = Cli::command();
        let onboard = cmd
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == "onboard")
            .expect("onboard subcommand must exist");

        let has_model_flag = onboard
            .get_arguments()
            .any(|arg| arg.get_id().as_str() == "model" && arg.get_long() == Some("model"));

        assert!(
            has_model_flag,
            "onboard help should include --model for quick setup overrides"
        );
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn onboard_cli_accepts_model_provider_and_api_key_in_quick_mode() {
        let cli = Cli::try_parse_from([
            "naraeclaw",
            "onboard",
            "--provider",
            "openrouter",
            "--model",
            "custom-model-946",
            "--api-key",
            "sk-issue946",
        ])
        .expect("quick onboard invocation should parse");

        match cli.command {
            Commands::Onboard {
                force,
                channels_only,
                api_key,
                provider,
                model,
                ..
            } => {
                assert!(!force);
                assert!(!channels_only);
                assert_eq!(provider.as_deref(), Some("openrouter"));
                assert_eq!(model.as_deref(), Some("custom-model-946"));
                assert_eq!(api_key.as_deref(), Some("sk-issue946"));
            }
            other => panic!("expected onboard command, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn completions_cli_parses_supported_shells() {
        for shell in ["bash", "fish", "zsh", "powershell", "elvish"] {
            let cli = Cli::try_parse_from(["naraeclaw", "completions", shell])
                .expect("completions invocation should parse");
            match cli.command {
                Commands::Completions { .. } => {}
                other => panic!("expected completions command, got {other:?}"),
            }
        }
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn completion_generation_mentions_binary_name() {
        let mut output = Vec::new();
        write_shell_completion(CompletionShell::Bash, &mut output)
            .expect("completion generation should succeed");
        let script = String::from_utf8(output).expect("completion output should be valid utf-8");
        assert!(
            script.contains("naraeclaw"),
            "completion script should reference binary name"
        );
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn onboard_cli_accepts_force_flag() {
        let cli = Cli::try_parse_from(["naraeclaw", "onboard", "--force"])
            .expect("onboard --force should parse");

        match cli.command {
            Commands::Onboard { force, .. } => assert!(force),
            other => panic!("expected onboard command, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn onboard_cli_rejects_removed_interactive_flag() {
        // --interactive was removed; onboard auto-detects TTY instead.
        assert!(Cli::try_parse_from(["naraeclaw", "onboard", "--interactive"]).is_err());
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn onboard_cli_parses_quick_flag() {
        let cli = Cli::try_parse_from(["naraeclaw", "onboard", "--quick"])
            .expect("onboard --quick should parse");

        match cli.command {
            Commands::Onboard { quick, .. } => assert!(quick),
            other => panic!("expected onboard command, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn onboard_cli_quick_and_channels_only_conflict() {
        // --quick and --channels-only should both parse at the CLI level
        // (the conflict is checked at runtime), but we verify both flags parse.
        let cli = Cli::try_parse_from(["naraeclaw", "onboard", "--quick", "--channels-only"]);
        assert!(
            cli.is_ok(),
            "--quick --channels-only should parse at CLI level"
        );
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn onboard_cli_bare_parses() {
        let cli = Cli::try_parse_from(["naraeclaw", "onboard"]).expect("bare onboard should parse");

        match cli.command {
            Commands::Onboard { .. } => {}
            other => panic!("expected onboard command, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn cli_parses_estop_default_engage() {
        let cli = Cli::try_parse_from(["naraeclaw", "estop"]).expect("estop command should parse");

        match cli.command {
            Commands::Estop {
                estop_command,
                level,
                domains,
                tools,
            } => {
                assert!(estop_command.is_none());
                assert!(level.is_none());
                assert!(domains.is_empty());
                assert!(tools.is_empty());
            }
            other => panic!("expected estop command, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn cli_parses_estop_resume_domain() {
        let cli = Cli::try_parse_from(["naraeclaw", "estop", "resume", "--domain", "*.chase.com"])
            .expect("estop resume command should parse");

        match cli.command {
            Commands::Estop {
                estop_command: Some(EstopSubcommands::Resume { domains, .. }),
                ..
            } => assert_eq!(domains, vec!["*.chase.com".to_string()]),
            other => panic!("expected estop resume command, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn agent_command_parses_with_temperature() {
        let cli = Cli::try_parse_from(["naraeclaw", "agent", "--temperature", "0.5"])
            .expect("agent command with temperature should parse");

        match cli.command {
            Commands::Agent { temperature, .. } => {
                assert_eq!(temperature, Some(0.5));
            }
            other => panic!("expected agent command, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn agent_command_parses_without_temperature() {
        let cli = Cli::try_parse_from(["naraeclaw", "agent", "--message", "hello"])
            .expect("agent command without temperature should parse");

        match cli.command {
            Commands::Agent { temperature, .. } => {
                assert_eq!(temperature, None);
            }
            other => panic!("expected agent command, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn agent_command_parses_session_state_file() {
        let cli =
            Cli::try_parse_from(["naraeclaw", "agent", "--session-state-file", "session.json"])
                .expect("agent command with session state file should parse");

        match cli.command {
            Commands::Agent {
                session_state_file, ..
            } => {
                assert_eq!(session_state_file, Some(PathBuf::from("session.json")));
            }
            other => panic!("expected agent command, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn agent_fallback_uses_config_default_temperature() {
        // Test that when user doesn't provide --temperature,
        // the fallback logic works correctly
        let mut config = Config::default(); // default_temperature = 0.7
        config.default_temperature = 1.5;

        // Simulate None temperature (user didn't provide --temperature)
        let user_temperature: Option<f64> = std::hint::black_box(None);
        let final_temperature = user_temperature.unwrap_or(config.default_temperature);

        assert!((final_temperature - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    #[cfg(feature = "agent-runtime")]
    fn agent_fallback_uses_hardcoded_when_config_uses_default() {
        // Test that when config uses default value (0.7), fallback still works
        let config = Config::default(); // default_temperature = 0.7

        // Simulate None temperature (user didn't provide --temperature)
        let user_temperature: Option<f64> = std::hint::black_box(None);
        let final_temperature = user_temperature.unwrap_or(config.default_temperature);

        assert!((final_temperature - 0.7).abs() < f64::EPSILON);
    }
}

#[cfg(not(feature = "tui-onboarding"))]
#[allow(clippy::unused_async)]
async fn run_tui_if_enabled() -> anyhow::Result<()> {
    anyhow::bail!("TUI onboarding feature is not enabled. Rebuild with --features tui-onboarding")
}
