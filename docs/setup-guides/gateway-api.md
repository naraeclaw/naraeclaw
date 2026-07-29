# Gateway API 연결

최종 검증: **2026-07-29**

NaraeClaw는 `naraeclaw agent`(CLI)와 `naraeclaw gateway`(HTTP/WebSocket API)로 동작합니다.

> **참고**: Desktop(Tauri) 앱과 Web UI는 2026-05-05에 제거되었습니다.
> CLI + 게이트웨이 API가 유일한 제품 표면입니다.
> 현재 바이너리에 보이는 `desktop` 명령은 레거시 호환 launcher이며 지원 표면이
> 아닙니다.

## 기본 구성

```text
외부 클라이언트
      │
      │ HTTP / WebSocket
      ▼
naraeclaw gateway (포트 42617)
      │
      │ 내장 에이전트 세션
      ▼
naraeclaw-runtime agent loop
      │
      │ managed MCP (safe profile)
      ▼
ByoriDB (workspace-scoped knowledge)
```

## 게이트웨이 시작

```bash
# 포그라운드 실행 (`start`는 생략 가능)
naraeclaw gateway
naraeclaw gateway start

# 전체 daemon을 서비스로 등록 후 백그라운드 실행
naraeclaw service install
naraeclaw service start
```

## 포트 설정

기본 포트는 `42617`입니다. 변경하려면:

```bash
# 환경 변수
export NARAECLAW_GATEWAY_PORT=8080

# 또는 config.toml
[gateway]
port = 8080
```

## 주요 엔드포인트

| 경로 | 설명 |
|---|---|
| `GET /health` | 공개 게이트웨이 및 런타임 health snapshot |
| `GET /api/status` | 시스템 상태와 유효 ByoriDB provider/space/MCP readiness (`knowledge`) |
| `GET /api/health` | 구성요소별 runtime health snapshot |
| `WebSocket /ws/chat` | 에이전트 채팅 세션 |

전체 REST/WebSocket 경로와 인증 경계는
`crates/naraeclaw-gateway/src/lib.rs`의 router 구성이 source of truth입니다.

## 트러블슈팅

| 증상 | 확인 사항 |
|---|---|
| 포트 충돌 | `lsof -i :42617` 으로 점유 프로세스 확인 |
| 서비스 미시작 | `naraeclaw service status` 확인 |
| 연결 거부 | `naraeclaw self-test` 실행 |
| 지식 도구 비정상 | `naraeclaw knowledge status` 실행 |
