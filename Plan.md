# NaraeClaw 작업 계획

> 최종 목표: ZeroClaw 포크를 **텔레그램 중심·한국어 우선·경량·안전** 에이전트로 전환한다.
> 업데이트: 2026-04-13 (Gemini 아키텍처 점검 결과 반영)

---

## 전체 우선순위 요약

| # | 작업 | 위험도 | 상태 | 담당 |
|---|------|--------|------|------|
| ✅ 3 | 시스템 프롬프트 한국어화 | 저위험 | **완료** | Claude |
| ✅ 4 | CLI help 텍스트 한국어화 | 저위험 | **완료** | Claude |
| 1 | unsafe 환경 변수 → OnceLock 전환 | 중위험 | 대기 | — |
| 2 | Default feature 경량화 | 중위험 | 대기 | — |
| 3 | 하드코딩 설정 유연화 (Provider URL) | 중위험 | 대기 | — |
| 4 | Telegram polling → webhook 전환 | 고위험 | **진행 중** | Codex |
| 5 | zeroize 패턴 도입 | 중위험 | 대기 | — |
| 6 | 보안 메커니즘 통합 (LeakDetector) | 고위험 | 대기 | — |

**실행 원칙**: 모든 작업은 worktree 브랜치에서 진행. master 직접 커밋 금지.

---

## 작업 1: unsafe 환경 변수 → OnceLock 전환

**위험도**: 중위험 | **출처**: Gemini 아키텍처 점검
**파일**: `crates/zeroclaw-runtime/`, `crates/zeroclaw-providers/`, `crates/zeroclaw-tools/`, `src/main.rs`

### 문제

Tokio 비동기 런타임에서 `unsafe { std::env::set_var(...) }` 사용은 **정의되지 않은 동작(UB)** 을 유발할 수 있다.
다른 스레드가 환경 변수를 읽는 도중 수정이 일어나면 메모리 오염·레이스 컨디션 발생 가능.

### 접근 방법

```rust
// 변경 전
unsafe { std::env::set_var("OPENAI_API_KEY", &key); }

// 변경 후 — 프로세스 시작 시 1회 초기화, 이후 불변
static OPENAI_KEY: OnceLock<String> = OnceLock::new();
OPENAI_KEY.get_or_init(|| key.clone());
```

- 런타임 중 동적으로 바꿔야 하는 값은 `Arc<RwLock<T>>` 또는 채널로 전달
- 테스트 코드의 `set_var` / `remove_var` 는 `#[serial_test]` 또는 환경 격리로 대체
- `set_var`를 grep으로 전수 조사 후 각각 적절한 패턴 결정

### 완료 기준

- [ ] 프로덕션 코드에서 `unsafe { set_var }` 0건
- [ ] `cargo test --lib` 통과
- [ ] `cargo clippy --all-targets -- -D warnings` 통과

---

## 작업 2: Default Feature 경량화

**위험도**: 중위험 | **출처**: 초기 계획 + Gemini 확인 (공격 표면 축소)
**파일**: `Cargo.toml` (루트), `crates/zeroclaw-channels/Cargo.toml`

### 문제

현재 default에 채널 **24개** 포함 → 바이너리 크기 증가, 불필요한 공격 표면.

### 접근 방법

**제거 대상 (default → opt-in)**:

| 그룹 | 채널 |
|------|------|
| `channels-cn` | qq, dingtalk, mochat, wecom, wati |
| `channels-social` | bluesky, twitter, reddit |
| `channels-legacy` | irc, imessage, linq, lark |
| 기타 opt-in | voice-call |

**유지 (default)**:
telegram, discord, slack, signal, mattermost, email, webhook, acp-server, whatsapp-cloud, nextcloud, notion

**추가할 feature 그룹**:
```toml
channels-cn     = ["channel-qq", "channel-dingtalk", ...]
channels-social = ["channel-bluesky", "channel-twitter", ...]
channels-legacy = ["channel-irc", "channel-imessage", ...]
channels-full   = ["agent-runtime", "channels-cn", "channels-social", "channels-legacy", "channel-voice-call"]
```

### 완료 기준

- [ ] `cargo build` (default features) 성공
- [ ] `cargo build --features channels-full` 성공
- [ ] `cargo test` 통과
- [ ] 빌드 시간 단축 확인

---

## 작업 3: 하드코딩 설정 유연화

**위험도**: 중위험 | **출처**: Gemini 아키텍처 점검
**파일**: `crates/zeroclaw-providers/`

### 문제

Provider별 베이스 URL, 모델명 등이 코드 내 문자열 리터럴로 하드코딩되어 있음.
코드 수정 없이 설정 파일만으로 Provider를 제어할 수 없음.

### 접근 방법

- `zeroclaw-config`의 각 Provider 설정 구조체에 `base_url: Option<String>` 필드 추가
- 기본값은 현재 하드코딩된 값을 `Default` 구현으로 이동
- `OPENAI_API_KEY`, `ANTHROPIC_API_KEY` 등 환경 변수 키 이름을 중앙 상수 모듈로 집중

```rust
// crates/zeroclaw-providers/src/constants.rs (신규)
pub const ENV_OPENAI_API_KEY: &str = "OPENAI_API_KEY";
pub const ENV_ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
```

### 완료 기준

- [ ] Provider 베이스 URL이 설정 파일로 오버라이드 가능
- [ ] 환경 변수 키 이름이 중앙 상수로 관리됨
- [ ] `cargo test` 통과

---

## 작업 4: Telegram Polling → Webhook 전환

**위험도**: 고위험 | **출처**: 초기 계획
**파일**: `crates/zeroclaw-channels/src/telegram.rs` 외 7개 파일
**담당**: Codex (진행 중, `codex/telegram-webhook` 브랜치)

### 현황

Codex가 다음 파일을 수정 중 (미커밋):
- `telegram.rs` (+309줄) — webhook 수신 서버 구현
- `schema.rs` (+35줄) — `webhook_url`, `webhook_listen_addr`, `webhook_path`, `webhook_secret_token` 필드 추가
- `orchestrator/mod.rs`, `daemon/mod.rs`, `onboard/wizard.rs` 등 (+40줄)

### 구현 구조 (설계)

```rust
// TelegramConfig 신규 필드
webhook_url: Option<String>,           // None이면 polling 모드
webhook_listen_addr: String,           // 기본값 "0.0.0.0:8443"
webhook_path: String,                  // 기본값 "/telegram/webhook"
webhook_secret_token: Option<String>,  // X-Telegram-Bot-Api-Secret-Token

// listen() 분기
pub async fn listen(&self) -> anyhow::Result<()> {
    if self.config.webhook_url.is_some() {
        self.listen_webhook().await
    } else {
        self.listen_polling().await  // 기존 코드 유지
    }
}
```

### 완료 기준

- [ ] `webhook_url` 설정 시 webhook 모드 동작
- [ ] secret token 검증 동작
- [ ] polling 모드 기존과 동일
- [ ] `cargo test --lib` 통과

---

## 작업 5: zeroize 패턴 도입

**위험도**: 중위험 | **출처**: Gemini 아키텍처 점검
**파일**: `crates/zeroclaw-config/`, `crates/zeroclaw-providers/`, `crates/zeroclaw-memory/`

### 문제

API 키, 봇 토큰, 평문 세션 데이터가 메모리에 `String`으로 보유됨.
비정상 종료 시 메모리 덤프를 통한 정보 유출 가능.

### 접근 방법

- `zeroize` 크레이트 의존성 추가
- `#[secret]` 마킹된 필드(API 키, 토큰)에 `Zeroize` derive 적용
- 세션 데이터를 담는 구조체에 `ZeroizeOnDrop` 적용

```toml
# Cargo.toml
zeroize = { version = "1", features = ["derive"] }
```

```rust
#[derive(Zeroize, ZeroizeOnDrop)]
struct ApiCredentials {
    api_key: String,
    token: String,
}
```

### 완료 기준

- [ ] `#[secret]` 필드에 zeroize 적용 완료
- [ ] Drop 시 메모리 zeroing 동작 확인 (단위 테스트)
- [ ] `cargo test` 통과

---

## 작업 6: 보안 메커니즘 통합 (LeakDetector)

**위험도**: 고위험 | **출처**: Gemini 아키텍처 점검
**파일**: `zeroclaw-runtime/src/security/`, `zeroclaw-channels/src/`

### 문제

- `LeakDetector` (채널용): 채널 출력에서 민감 정보 필터링
- `scrub_credentials` (에이전트 루프용): 에이전트 응답에서 자격증명 제거

두 로직이 파편화되어 있어 한쪽을 통과하면 다른 쪽에서 유출될 수 있음.
또한 Base64·Hex 인코딩된 데이터, 파편화된 스트리밍 출력에 대한 방어 검증 필요.

### 접근 방법

- `zeroclaw-runtime/src/security/` 내에 `CredentialFilter` 단일 엔진 설계
- `LeakDetector`와 `scrub_credentials` 모두 이 엔진 호출로 통합
- 인코딩 변형(Base64, URL encoding) 탐지 로직 추가
- 스트리밍 청크 경계에서도 패턴 매칭 동작 확인

### ⚠️ 주의

`security/` 는 고위험 영역. 변경 전 기존 동작을 테스트로 충분히 고정한 후 진행.

### 완료 기준

- [ ] `LeakDetector`와 `scrub_credentials`가 단일 엔진 호출로 통합
- [ ] Base64/Hex 인코딩 자격증명 필터링 테스트 추가
- [ ] 스트리밍 청크 경계 테스트 추가
- [ ] 기존 보안 테스트 전부 통과

---

## 실행 순서

```
✅ 완료: CLI 한국어화, 시스템 프롬프트 한국어화

진행 중 (병렬):
  Codex  → 작업 4 (Telegram webhook)
  Claude → 작업 1 (unsafe set_var → OnceLock)  ← 다음

이후 순차:
  작업 2 (Feature 경량화)
  작업 3 (하드코딩 유연화)
  작업 5 (zeroize)
  작업 6 (보안 통합, 가장 복잡·신중)
```
