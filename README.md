# 나래클로 (NaraeClaw)

> 나래 — 날개의 고어. 가볍게, 빠르게.

**한국어 우선 경량 AI 에이전트 런타임.**  
텔레그램 등 메신저에서 즉시 응답하는 개인 AI 어시스턴트.

ZeroClaw 포크 — 불필요한 기능을 걷어내고 한국어 환경에 최적화.

---

## 특징

- **빠른 응답** — 텔레그램 Webhook 방식 (polling 지연 없음)
- **경량** — 필요한 채널/기능만 빌드
- **한국어 우선** — 기본 시스템 프롬프트, CLI 메시지 한국어
- **기능 유지** — SOP, 크론, 메모리, 스킬 등 핵심 기능 그대로

---

## 빌드

```bash
# 의존성: Rust 1.87+
cargo build --release

# 개발 모드 실행
cargo run -- onboard
```

## 실행

```bash
# 온보딩 마법사
naraeclaw onboard

# 에이전트 시작
naraeclaw agent
```

---

## 원본 프로젝트

이 프로젝트는 [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)의 포크입니다.  
라이선스: MIT OR Apache-2.0
