# Desktop ↔ Gateway 연결

NaraeClaw Desktop(Tauri 앱)은 `naraeclaw agent`를 sidecar 프로세스로 실행하고,
내장 gateway의 HTTP/WebSocket API에 연결하여 동작합니다.

## 아키텍처

```text
┌─────────────────────┐
│  Tauri Desktop App  │
│  (WebView + Rust)   │
│         │           │
│    localhost:42617   │
│         │           │
│  ┌──────▼────────┐  │
│  │ naraeclaw agent│  │  ← sidecar 프로세스
│  │  (gateway 내장)│  │
│  └───────────────┘  │
└─────────────────────┘
```

- Desktop 앱이 시작되면 `sidecar::spawn_agent()`가 `naraeclaw agent`를 자동 실행합니다.
- WebView는 `http://127.0.0.1:42617`로 gateway에 접근합니다.
- 앱 종료 시 `sidecar::shutdown_agent()`가 프로세스를 정리합니다.

## 포트 설정

기본 포트는 `42617`입니다. 변경하려면:

```bash
# 환경 변수
export NARAECLAW_GATEWAY_PORT=8080

# 또는 config.toml
[gateway]
port = 8080
```

Desktop 앱은 `NARAECLAW_GATEWAY_PORT` 환경 변수를 읽어 sidecar와 같은 포트를 사용합니다.

## Sidecar 바이너리 탐색 순서

1. Tauri 번들 sidecar: 실행 파일 옆 `naraeclaw-{target-triple}`
2. 실행 파일 옆 `naraeclaw`
3. `NARAECLAW_BIN` 환경 변수
4. 워크스페이스 `target/release/naraeclaw` 또는 `target/debug/naraeclaw` (개발 모드)
5. `PATH`에서 `naraeclaw`

## 페어링

Gateway에 페어링이 활성화된 경우, Desktop 앱은 시작 시 자동 페어링을 시도합니다.
localhost 접근이므로 admin 엔드포인트에 인증 없이 접근 가능합니다.

토큰은 Tauri store에 저장되어 이후 세션에서 재사용됩니다.

## 헬스 체크

Desktop 앱은 주기적으로 gateway 상태를 폴링합니다:

- `GET /api/status` — 시스템 상태 확인
- 연결 상태에 따라 트레이 아이콘이 변경됩니다 (idle/working/disconnected/error)

## 트러블슈팅

| 증상 | 확인 사항 |
|---|---|
| 앱 시작 후 빈 화면 | `naraeclaw` 바이너리가 PATH에 있는지 확인 |
| 연결 실패 | 포트 42617이 다른 프로세스에 점유되지 않았는지 확인 |
| sidecar 미시작 | `NARAECLAW_BIN` 환경 변수로 바이너리 경로 직접 지정 |
| 페어링 실패 | gateway 설정에서 `[gateway] pairing = true` 확인 |
