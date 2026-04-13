# NaraeClaw 작업 계획

> 최종 목표: ZeroClaw 포크를 **텔레그램 중심·한국어 우선·경량** 에이전트로 전환한다.
> 업데이트: 2026-04-13

---

## 우선순위 요약

| # | 작업 | 위험도 | 예상 규모 |
|---|------|--------|-----------|
| 1 | Telegram polling → webhook 전환 | 고위험 | 대 |
| 2 | Default feature 경량화 | 중위험 | 소 |
| 3 | 시스템 프롬프트 한국어화 | 저위험 | 중 |
| 4 | CLI help 텍스트 한국어화 | 저위험 | 소 |

---

## 작업 1: Telegram Polling → Webhook 전환

**위험도**: 고위험 (채널 핵심 로직 교체)
**파일**: `crates/zeroclaw-channels/src/telegram.rs`

### 현황

- `listen()` 메서드(라인 2780-2972)가 `getUpdates` 롱폴링으로 구현되어 있음
- polling timeout: **30초** (라인 2871) → 최대 30초 응답 지연 발생
- draft 업데이트 간격: **1000ms** (라인 372)
- webhook 관련 코드: **전무** (완전 신규 구현 필요)
- 409 충돌 시 백오프: **35초** (라인 2915)

### 접근 방법

Telegram Bot API는 polling과 webhook을 동시에 지원하지 않는다.
webhook을 설정하면 `getUpdates`가 에러를 반환하므로, 모드를 설정으로 선택하게 한다.

#### 단계별 구현

**1-A. Config 확장** (`crates/zeroclaw-channels/src/telegram.rs` 상단 Config 구조체)
```rust
// 추가할 설정 필드
webhook_url: Option<String>,     // None이면 polling 모드
webhook_secret: Option<String>,  // X-Telegram-Bot-Api-Secret-Token 검증용
webhook_port: u16,               // 기본값 8443 (Telegram 허용 포트 중 하나)
```

**1-B. Webhook 등록/해제 메서드 구현**
- `setWebhook` API 호출: `POST /bot{token}/setWebhook`
- `deleteWebhook` API 호출: 종료 시 cleanup
- allowed_updates, max_connections 파라미터 포함

**1-C. HTTP 서버 구현**
- `axum` 또는 기존 의존성 활용
- `POST /` 엔드포인트: Telegram이 보내는 Update JSON 수신
- `X-Telegram-Bot-Api-Secret-Token` 헤더 검증 (보안 필수)
- 수신한 Update를 기존 메시지 처리 파이프라인으로 전달

**1-D. `listen()` 분기 처리**
```rust
pub async fn listen(&self) -> anyhow::Result<()> {
    if self.config.webhook_url.is_some() {
        self.listen_webhook().await
    } else {
        self.listen_polling().await  // 기존 코드 유지
    }
}
```

#### 고려사항

- **TLS 필수**: Telegram webhook은 HTTPS만 허용 (자체 서명 인증서는 허용하되 공개키 등록 필요)
- **로컬 개발**: ngrok 또는 cloudflared tunnel 사용 안내 문서 추가
- **기존 polling 코드 유지**: 설정에 `webhook_url`이 없으면 polling 모드로 동작 (하위호환)
- **테스트**: `MockChannel` 인프라 활용, webhook payload fixture 추가 (`tests/fixtures/traces/`)

#### 완료 기준

- [ ] `webhook_url` 설정 시 자동으로 webhook 모드로 동작
- [ ] `setWebhook` / `deleteWebhook` 라이프사이클 정상 동작
- [ ] secret token 검증 통과 시에만 Update 처리
- [ ] polling 모드는 기존과 동일하게 동작
- [ ] `cargo test --test integration` 통과

---

## 작업 2: Default Feature 경량화

**위험도**: 중위험 (빌드 의존성 변경, feature flag 조건부 코드 영향)
**파일**: `Cargo.toml` (루트), `crates/zeroclaw-channels/Cargo.toml`

### 현황

현재 `agent-runtime` feature(default에 포함)가 채널 **24개**를 모두 포함:

```
channel-email, channel-telegram, channel-lark, channel-discord,
channel-slack, channel-signal, channel-mattermost, channel-irc,
channel-imessage, channel-dingtalk, channel-qq, channel-bluesky,
channel-twitter, channel-reddit, channel-notion, channel-linq,
channel-wati, channel-nextcloud, channel-mochat, channel-wecom,
channel-clawdtalk, channel-webhook, channel-acp-server,
channel-whatsapp-cloud, channel-voice-call
```

### 접근 방법

NaraeClaw의 주 타겟은 텔레그램이므로 default를 최소화하고, 나머지는 opt-in으로 변경.

#### 제거 대상 (default에서 제외)

| 채널 | 제거 이유 |
|------|-----------|
| `channel-lark` | `prost` 의존성(protobuf) — 빌드 무거움, 한국 사용자 불필요 |
| `channel-qq` | 중국 시장 전용 |
| `channel-dingtalk` | 중국 시장 전용 |
| `channel-mochat` | 중국 시장 전용 |
| `channel-wecom` | 중국 시장 전용 |
| `channel-wati` | WhatsApp 리셀러, 틈새 시장 |
| `channel-linq` | 틈새 시장 |
| `channel-bluesky` | 소셜 미디어, 에이전트 채널로 부적합 |
| `channel-twitter` | 소셜 미디어, API 비용 높음 |
| `channel-reddit` | 소셜 미디어 |
| `channel-irc` | 레거시 프로토콜 |
| `channel-imessage` | macOS 전용, 범용성 없음 |
| `channel-voice-call` | 복잡한 의존성, 선택적 기능 |

#### 유지 대상 (default 포함)

| 채널 | 유지 이유 |
|------|-----------|
| `channel-telegram` | 주 채널 |
| `channel-discord` | 개발자 커뮤니티 표준 |
| `channel-slack` | 기업 환경 표준 |
| `channel-signal` | 보안 메시징 |
| `channel-mattermost` | 자체 호스팅 Slack 대안 |
| `channel-email` | 범용 알림 채널 |
| `channel-webhook` | 통합 허브 역할 |
| `channel-acp-server` | 에이전트 간 통신 |
| `channel-whatsapp-cloud` | 한국 외 광범위 사용 |
| `channel-nextcloud` | 자체 호스팅 생태계 |
| `channel-notion` | 지식 관리 통합 |

#### 변경 내용

루트 `Cargo.toml`의 `agent-runtime` feature 수정:
```toml
agent-runtime = [
    "dep:zeroclaw-runtime", "dep:zeroclaw-channels", "dep:zeroclaw-tools",
    # 유지 채널만 포함
    "channel-telegram", "channel-discord", "channel-slack",
    "channel-signal", "channel-mattermost", "channel-email",
    "channel-webhook", "channel-acp-server", "channel-whatsapp-cloud",
    "channel-nextcloud", "channel-notion",
    # ... 기타 비채널 feature ...
]

# 제거된 채널은 별도 feature 그룹으로 분리
channels-cn = [
    "channel-qq", "channel-dingtalk", "channel-mochat",
    "channel-wecom", "channel-wati",
]
channels-social = [
    "channel-bluesky", "channel-twitter", "channel-reddit",
]
channels-legacy = [
    "channel-irc", "channel-imessage", "channel-linq",
    "channel-lark",
]
channels-full = [
    "agent-runtime", "channels-cn", "channels-social",
    "channels-legacy", "channel-voice-call",
]
```

#### 완료 기준

- [ ] `cargo build` (default features) 성공
- [ ] `cargo build --features channels-full` 성공
- [ ] `cargo test` 통과
- [ ] 빌드 시간 측정 후 단축 확인 (before/after 기록)

---

## 작업 3: 시스템 프롬프트 한국어화

**위험도**: 저위험 (동작 변경 없음, 텍스트 교체)
**파일**: `crates/zeroclaw-runtime/src/agent/system_prompt.rs`

### 현황

`system_prompt.rs`(라인 1-344)의 모든 텍스트가 영어.
구조: `build_system_prompt()` 함수가 여러 섹션을 `push_str`로 조립.

주요 섹션:
- Anti-narration 규칙 (라인 123-131)
- Tool Honesty (라인 134-139)
- Task 지침 (라인 181-195)
- Safety 규칙 (라인 198-222)
- Project Context (라인 242-280)
- Date & Time (라인 283-289)
- Channel Capabilities (라인 301-324)

### 접근 방법

언어 설정을 config에서 읽어 한국어/영어를 선택하는 방식.
초기에는 단순하게 **한국어를 기본값으로 하드코딩**하고, 추후 config 연동.

#### 번역 원칙

- LLM이 이해하기 쉬운 명확한 지시문 사용
- 기술 용어(tool, skill, SOP 등)는 영어 그대로 유지
- 지나치게 격식체는 피하되 명확성 우선

#### 주요 섹션 번역 방향

```
"## CRITICAL: No Tool Narration"
→ "## 중요: Tool 사용 발화 금지"

"NEVER narrate, announce, describe, or explain your tool usage"
→ "Tool 사용을 설명하거나 예고하지 마세요."

"## Task"
→ "## 작업 지침"

"## Safety"  
→ "## 안전 규칙"
```

#### 완료 기준

- [ ] 모든 사용자 대면 섹션 한국어 번역 완료
- [ ] 기술 용어(tool name, API 등)는 영어 유지
- [ ] `cargo test --lib` 통과 (프롬프트 빌드 로직 변경 없음)
- [ ] 에이전트 실행 후 프롬프트 출력 육안 확인

---

## 작업 4: CLI Help 텍스트 한국어화

**위험도**: 저위험 (사용자 인터페이스 텍스트만 변경)
**파일**: `src/main.rs`

### 현황

- Clap `#[command]` / `#[arg]` 매크로의 doc-comment로 help 텍스트 생성
- 바이너리명은 `naraeclaw`로 이미 변경됨 (라인 191: `name = "zeroclaw"` → 확인 필요)
- `print_no_command_help()` 함수(라인 52-65)에도 영어 텍스트 존재

### 변경 대상

**Cli 구조체** (라인 189-200):
```rust
// 변경 전
/// `ZeroClaw` - Zero overhead. Zero compromise. 100% Rust.
#[command(about = "The fastest, smallest AI assistant.")]

// 변경 후
/// NaraeClaw — 빠르고 가벼운 AI 에이전트
#[command(name = "naraeclaw")]
#[command(about = "빠르고 가벼운 한국어 AI 에이전트")]
```

**각 서브커맨드 번역 대상**:
- `Onboard`: "Initialize your workspace and configuration" → "작업 환경 초기화 및 설정"
- `Agent`: long_about 전체 번역
- `Gateway`: REST/WebSocket 게이트웨이 설명 번역
- `Daemon`: 데몬 모드 설명 번역

**`print_no_command_help()`** (라인 52-65):
```rust
// 변경 전
println!("No command provided.");
println!("Try `zeroclaw onboard` to initialize your workspace.");

// 변경 후
println!("명령어를 입력해주세요.");
println!("`naraeclaw onboard` 로 작업 환경을 초기화할 수 있습니다.");
```

#### 완료 기준

- [ ] `naraeclaw --help` 출력이 한국어로 표시
- [ ] `naraeclaw agent --help` 출력이 한국어로 표시
- [ ] `naraeclaw onboard --help` 출력이 한국어로 표시
- [ ] `cargo test` 통과

---

## 실행 순서 권장

```
작업 4 (CLI, 저위험, 빠름)
  → 작업 3 (시스템 프롬프트, 저위험)
  → 작업 2 (Feature 경량화, 중위험)
  → 작업 1 (Webhook, 고위험, 가장 오래 걸림)
```

저위험 작업으로 워밍업 후, 고위험 작업에 집중하는 순서.
작업 1(Webhook)은 로컬 HTTPS 환경(ngrok 등) 준비가 필요하므로 별도 사전 준비 필요.

---

## 사전 준비 체크리스트

- [ ] `cargo build` 현재 성공 여부 확인
- [ ] `cargo test` 현재 통과율 확인 (기준선 확보)
- [ ] Webhook 테스트용 ngrok 또는 cloudflared 설치
- [ ] Telegram Bot Token 준비 (개발용 별도 봇 권장)
