# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 이 프로젝트에 대해

**NaraeClaw**는 [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)에서 출발한 포크로, macOS·Windows·Linux에서 동작하는 한국어 우선 경량 AI 에이전트 런타임입니다. CLI / Desktop / Web 표면을 우선으로 두고, 레거시 범위는 단계적으로 정리합니다.

현재 중점:
- ✅ **브랜딩 정리** — 사용자 노출 문구를 NaraeClaw 기준으로 통일
- ✅ **경량화** — 기본 범위에서 불필요한 채널·플러그인 제거
- ✅ **보안 강화** — `unsafe set_var` 제거, `OnceLock`/`zeroize`/`CredentialFilter` 유지

내부 크레이트명과 바이너리 이름은 `naraeclaw-*` / `naraeclaw`를 사용합니다. 기존 `ZEROCLAW_*` 환경변수는 호환 fallback으로만 유지합니다.

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

Rust edition 2024 워크스페이스. `naraeclaw` 바이너리(`src/main.rs`)는 기본적으로 `agent-runtime` feature가 켜져 있고, CLI / Desktop / Web 중심의 에이전트 서브시스템을 활성화합니다.

**메시지 흐름**: 수신 메시지 → `naraeclaw-channels` (전송 계층) → `naraeclaw-runtime/agent/` (에이전트 루프) → `naraeclaw-providers` (LLM 호출) → `naraeclaw-tools` (도구 실행) → 응답 전송

**핵심 확장 포인트** (`crates/naraeclaw-api/`):
- `provider.rs` — LLM Provider 트레이트
- `channel.rs` — 채널 트레이트
- `tool.rs` — 도구 트레이트
- `memory_traits.rs` — 메모리 백엔드 트레이트

**`naraeclaw-runtime/src/` 주요 서브시스템:**
- `agent/` — 에이전트 루프 핵심 (`loop_.rs`, `agent.rs`)
- `security/` — 접근제어 및 정책 (고위험, 신중히 수정)
- `cron/` — 크론 스케줄러
- `sop/` — SOP 엔진
- `skills/`, `skillforge/` — 스킬 시스템
- `onboard/` — TUI 온보딩 마법사

**채널** (`crates/naraeclaw-channels/`): 각 채널은 `channel-<이름>` Cargo feature로 게이팅됨. `orchestrator/`가 채널 생명주기와 미디어 파이프라인을 담당합니다.

**핵심 코드 위치:**
- `crates/naraeclaw-runtime/src/security/leak_detector.rs` — CredentialFilter 엔진
- `crates/naraeclaw-runtime/src/service/mod.rs` — macOS/Windows/Linux 서비스 관리
- `crates/naraeclaw-config/src/schema/mod.rs` — 설정 스키마 진입점
- `apps/tauri/` — Desktop sidecar 및 창 관리
- `web/` — Web UI와 설정 화면

**Config** (`crates/naraeclaw-config/`): TOML 기반, `Configurable` derive 매크로로 스키마 자동 생성. Config 키는 공개 계약 — 변경 시 기본값과 마이그레이션 경로 문서화 필수.

**테스트 인프라** (`tests/support/`): `MockProvider`, `MockChannel`, `EchoTool` 등 공유 목. JSON fixture replay는 `TraceLlmProvider` 사용 (`tests/fixtures/traces/`).

## 작업 우선순위 (V2 — 데스크탑 앱 포팅)

핵심 V1 작업은 모두 완료됨. 현재 우선순위:

1. **핵심 경로 안정화** — CLI / Desktop / Web의 기본 실행, 설정, 종료, 복원 흐름을 우선 검증
2. **브랜딩 정리** — 창 기본 표시, 아이콘·앱 이름, 사용자 노출 문자열을 NaraeClaw 기준으로 통일
3. **한국어 UI** — 웹 프론트엔드(`/web/src/`) 메뉴·버튼·안내 텍스트 한국어화
4. **검증 단순화** — fmt / check / clippy / 핵심 테스트를 빠르게 돌리는 루프 유지

자세한 계획: `Plan.md`

## 작업 워크플로우 — Worktree 규칙

**모든 작업은 worktree에서 시작한다.** master에 직접 커밋하지 않는다.

```bash
# 1. 작업 시작 — worktree + 브랜치 생성
git worktree add ../naraeclaw-<담당자> -b <담당자>/<작업명>
# 예) git worktree add ../naraeclaw-claude -b claude/feature-lightweight

# 2. 해당 worktree 안에서만 작업·커밋

# 3. 완료 후 master에 머지
git merge <브랜치명>

# 4. worktree·브랜치 정리
git worktree remove ../naraeclaw-<담당자>
git branch -d <브랜치명>
```

**브랜치 네이밍:**
- `claude/<작업명>` — Claude(이 대화)가 담당
- `codex/<작업명>` — Codex가 담당
- `gemini/<작업명>` — Gemini가 담당

**Worktree 경로 규칙:**
- 메인: `~/opensource/naraeclaw` (master, 리뷰·머지 전용)
- 에이전트: `~/opensource/naraeclaw-<담당자>` (작업 공간)

**병렬 작업 시 파일 충돌 최소화:**
- 한 작업 단위는 수정 파일이 서로 겹치지 않도록 설계한다
- 겹치는 파일이 불가피하면 순차 진행(앞 작업 머지 후 다음 시작)

## 작업 워크플로우 — Worktree 규칙

**모든 작업은 worktree에서 시작한다.** master에 직접 커밋하지 않는다.

```bash
# 1. 작업 시작 — worktree + 브랜치 생성
git worktree add ../naraeclaw-<담당자> -b <담당자>/<작업명>
# 예) git worktree add ../naraeclaw-claude -b claude/feature-lightweight

# 2. 해당 worktree 안에서만 작업·커밋

# 3. 완료 후 master에 머지
git merge <브랜치명>

# 4. worktree·브랜치 정리
git worktree remove ../naraeclaw-<담당자>
git branch -d <브랜치명>
```

**브랜치 네이밍:**
- `claude/<작업명>` — Claude(이 대화)가 담당
- `codex/<작업명>` — Codex가 담당
- `gemini/<작업명>` — Gemini가 담당

**Worktree 경로 규칙:**
- 메인: `~/opensource/naraeclaw` (master, 리뷰·머지 전용)
- 에이전트: `~/opensource/naraeclaw-<담당자>` (작업 공간)

**병렬 작업 시 파일 충돌 최소화:**
- 한 작업 단위는 수정 파일이 서로 겹치지 않도록 설계한다
- 겹치는 파일이 불가피하면 순차 진행(앞 작업 머지 후 다음 시작)

## 리스크 티어

- **저위험**: docs, 테스트, 설정값 조정
- **중위험**: `crates/*/src/**` 동작 변경
- **고위험**: `naraeclaw-runtime/src/security/`, `naraeclaw-gateway/src/`, `naraeclaw-tools/src/`
