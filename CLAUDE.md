# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 이 프로젝트에 대해

**NaraeClaw**는 [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) 포크로, 한국어 우선·경량화를 목표로 합니다.

주요 개선 목표:
1. **텔레그램 Webhook 전환** — `crates/zeroclaw-channels/src/telegram.rs` polling → webhook (응답 지연 제거)
2. **경량화** — 기본 feature에서 불필요한 채널 제거
3. **한국어화** — 시스템 프롬프트, CLI 메시지, 문서 한국어

내부 크레이트명은 여전히 `zeroclaw-*`이나, 바이너리 이름은 `naraeclaw`입니다.

## Commands

```bash
# 포맷
cargo fmt --all

# 린트
cargo clippy --all-targets -- -D warnings

# 전체 테스트
cargo test

# 단위 테스트만 (빠름)
cargo test --lib

# 테스트 레벨별 실행
cargo test --test component
cargo test --test integration
cargo test --test system

# 특정 테스트 필터
cargo test --test integration <테스트명>

# 라이브 테스트 (실제 API 키 필요, #[ignore] 표시)
cargo test --test live -- --ignored

# 개발 모드 실행
cargo run -- onboard
cargo run -- agent

# Justfile 단축키 (just 설치 필요)
just ci       # fmt-check + lint + test
just fmt      # cargo fmt --all
just dev      # cargo run --
```

## 아키텍처

Rust edition 2024 워크스페이스. `naraeclaw` 바이너리(`src/main.rs`)는 기본적으로 `agent-runtime` feature가 켜져 있고, 이것이 대부분의 에이전트 서브시스템을 활성화합니다.

**메시지 흐름**: 수신 메시지 → `zeroclaw-channels` (전송 계층) → `zeroclaw-runtime/agent/` (에이전트 루프) → `zeroclaw-providers` (LLM 호출) → `zeroclaw-tools` (도구 실행) → 응답 전송

**핵심 확장 포인트** (`crates/zeroclaw-api/`):
- `provider.rs` — LLM Provider 트레이트
- `channel.rs` — 채널 트레이트
- `tool.rs` — 도구 트레이트
- `memory_traits.rs` — 메모리 백엔드 트레이트

**`zeroclaw-runtime/src/` 주요 서브시스템:**
- `agent/` — 에이전트 루프 핵심 (`loop_.rs`, `agent.rs`)
- `security/` — 접근제어 및 정책 (고위험, 신중히 수정)
- `cron/` — 크론 스케줄러
- `sop/` — SOP 엔진
- `skills/`, `skillforge/` — 스킬 시스템
- `onboard/` — TUI 온보딩 마법사

**채널** (`crates/zeroclaw-channels/`): 각 채널은 `channel-<이름>` Cargo feature로 게이팅됨. `orchestrator/`가 채널 생명주기와 미디어 파이프라인 담당.

**텔레그램 지연 관련 코드 위치:**
- `crates/zeroclaw-channels/src/telegram.rs:2871` — polling timeout 30초 (webhook으로 교체 대상)
- `crates/zeroclaw-channels/src/telegram.rs:372` — draft 업데이트 간격 1000ms
- `crates/zeroclaw-channels/src/orchestrator/mod.rs:1844` — 메모리 recall (매 메시지마다 DB 쿼리)

**Config** (`crates/zeroclaw-config/`): TOML 기반, `Configurable` derive 매크로로 스키마 자동 생성. Config 키는 공개 계약 — 변경 시 기본값과 마이그레이션 경로 문서화 필수.

**테스트 인프라** (`tests/support/`): `MockProvider`, `MockChannel`, `EchoTool` 등 공유 목. JSON fixture replay는 `TraceLlmProvider` 사용 (`tests/fixtures/traces/`).

## 작업 우선순위

1. `telegram.rs` polling → webhook 전환
2. `Cargo.toml` default feature에서 불필요한 채널 제거
3. `zeroclaw-runtime/src/agent/system_prompt.rs` 한국어 기본 프롬프트
4. CLI help 텍스트 한국어화

## 리스크 티어

- **저위험**: docs, 테스트, 설정값 조정
- **중위험**: `crates/*/src/**` 동작 변경
- **고위험**: `zeroclaw-runtime/src/security/`, `zeroclaw-gateway/src/`, `zeroclaw-tools/src/`
