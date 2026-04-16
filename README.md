# 나래클로 (NaraeClaw)

> 나래 — 날개의 고어. 가볍게, 빠르게.

**한국어 우선 경량 AI 에이전트 런타임.**  
텔레그램, CLI, Web/Desktop에서 즉시 응답하는 개인 AI 어시스턴트.

나래클로는 개인 서버와 작업 환경을 관리하기 위한 경량 에이전트입니다.
인프라 상태 확인, 반복 운영 작업 자동화, 장기 메모리 기반 개인 지식 관리를 한 곳에서 다룹니다.

Zeroclaw 포크 — 불필요한 기능을 제거하고 한국어 환경, 서버 운영, 개인 지식 관리에 맞게 다듬고 있습니다.

---

## 핵심 특징

- **🚀 빠른 응답** — 텔레그램 Webhook 방식 지원 (Polling 지연 없음)
- **🪶 경량화** — 필요한 채널만 선택적으로 빌드 가능 (Feature-gate 최적화)
- **🇰🇷 한국어 우선** — 시스템 프롬프트, CLI 가이드, 도움말 메시지 완벽 한글화
- **🛡️ 강력한 보안** — `CredentialFilter`(통합 유출 방지), `SecretStore`(AEAD 암호화), `Zeroize`(메모리 안전) 적용
- **🧩 기능 확장** — SOP, 크론, 장기 메모리, 스킬 등 강력한 에이전트 기능 탑재

---

## 사용 사례

- 텔레그램/Web UI를 통한 서버 상태 확인 및 작업 요청
- 크론/SOP 기반 반복 운영 작업 자동화
- 장기 메모리에 운영 노트, 장애 이력, 개인 지식 저장
- 파일, 셸, HTTP, 브라우저 도구를 조합한 로컬 작업 보조
- 민감 정보 필터링과 감사 로그 기반 안전한 에이전트 운영

현재는 개인 운영자와 소규모 서버 환경을 우선 대상으로 하며, 대규모 멀티테넌트 SaaS보다 단순하고 직접 제어 가능한 구성을 지향합니다.

---

## 빌드 및 설치

### 기본 빌드 (경량 모드)
기본적으로 텔레그램, 디스코드, 슬랙 등 주요 채널만 포함됩니다.
```bash
# 의존성: Rust 1.87+
cargo build --release
```

---

## 실행 가이드

### 1. 온보딩 (설정 마법사)
설정 파일을 생성하고 API 키를 등록합니다. 모든 과정은 한국어로 진행됩니다.
```bash
naraeclaw onboard
```

### 2. 텔레그램 웹훅(Webhook) 설정
`config.toml`에서 다음 설정을 추가하여 웹훅 모드로 실행할 수 있습니다.
```toml
[channels_config.telegram]
webhook_url = "https://your-domain.com/telegram/webhook"
listen_addr = "0.0.0.0:8443"
webhook_secret_token = "your-secret-token" # 선택 사항
```

### 3. 에이전트 시작
```bash
naraeclaw agent
```

---

## 보안 및 데이터 보호

나래클로는 개인 어시스턴트의 특성을 고려하여 강력한 보안 레이어를 제공합니다.
- **Credential Filter**: 에이전트가 실수로 API 키나 비밀번호를 답변에 포함하면, 외부로 전송되기 전 자동으로 탐지하여 차단합니다 (Base64/Hex/URL 인코딩 탐지 포함).
- **Secret Store**: API 키와 토큰은 디스크에 평문으로 저장되지 않으며, `ChaCha20-Poly1305` 알고리즘으로 암호화되어 관리됩니다.
- **Zeroize**: 민감한 데이터는 사용이 끝난 즉시 메모리에서 완전히 소거(Zeroing)되어 메모리 덤프 취약점을 방어합니다.

---

## 라이선스 및 출처

나래클로는 [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)에서 출발한 포크입니다.

- 라이선스: `MIT OR Apache-2.0`
- 원본 프로젝트: ZeroClaw, Copyright 2025 ZeroClaw Labs
- 포크 및 변경분: NaraeClaw contributors
- 자세한 고지: [NOTICE](NOTICE), [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)

NaraeClaw는 공식 Zeroclaw 프로젝트가 아니며, upstream 프로젝트와의 제휴나 보증을 의미하지 않습니다.
