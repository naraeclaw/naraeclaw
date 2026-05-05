# CLI ↔ Gateway 연결

NaraeClaw는 `naraeclaw agent`(CLI)와 `naraeclaw gateway`(HTTP/WebSocket API)로 동작합니다.

> **참고**: Desktop(Tauri) 앱과 Web UI는 2026-05-05에 제거되었습니다.
> CLI + 게이트웨이 API가 유일한 제품 표면입니다.

## 기본 구성

```text
외부 클라이언트
      │
      │ HTTP / WebSocket
      ▼
naraeclaw gateway (포트 42617)
      │
      │ 내부 에이전트 세션
      ▼
naraeclaw agent (에이전트 루프)
```

## 게이트웨이 시작

```bash
# 포그라운드 실행
naraeclaw gateway

# 서비스로 등록 후 백그라운드 실행
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
| `GET /api/status` | 시스템 상태 |
| `WebSocket /ws` | 에이전트 세션 (ACP 프로토콜) |

## 트러블슈팅

| 증상 | 확인 사항 |
|---|---|
| 포트 충돌 | `lsof -i :42617` 으로 점유 프로세스 확인 |
| 서비스 미시작 | `naraeclaw service status` 확인 |
| 연결 거부 | `naraeclaw doctor` 실행 |
