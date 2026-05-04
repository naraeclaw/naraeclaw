# ZeroClaw 호환성 Fallback

NaraeClaw는 ZeroClaw에서 포크되었습니다. 기존 ZeroClaw 환경에서 마이그레이션하는
사용자를 위해 `ZEROCLAW_*` 환경 변수를 호환성 fallback으로 유지합니다.

## 환경 변수 Fallback

모든 `NARAECLAW_*` 환경 변수는 `ZEROCLAW_*` 변수를 fallback으로 읽습니다.

| 우선 (NaraeClaw) | Fallback (ZeroClaw) | 용도 |
|---|---|---|
| `NARAECLAW_API_KEY` | `ZEROCLAW_API_KEY` | LLM 프로바이더 API 키 |
| `NARAECLAW_WORKSPACE` | `ZEROCLAW_WORKSPACE` | 워크스페이스 디렉토리 |
| `NARAECLAW_CONFIG_DIR` | `ZEROCLAW_CONFIG_DIR` | 설정 디렉토리 |
| `NARAECLAW_GATEWAY_PORT` | `ZEROCLAW_GATEWAY_PORT` | 게이트웨이 포트 |
| `NARAECLAW_GATEWAY_HOST` | `ZEROCLAW_GATEWAY_HOST` | 게이트웨이 호스트 |
| `NARAECLAW_GATEWAY_TIMEOUT_SECS` | `ZEROCLAW_GATEWAY_TIMEOUT_SECS` | 게이트웨이 타임아웃 |
| `NARAECLAW_EXTRA_HEADERS` | `ZEROCLAW_EXTRA_HEADERS` | 추가 HTTP 헤더 |
| `NARAECLAW_AUDIT_SIGNING_KEY` | `ZEROCLAW_AUDIT_SIGNING_KEY` | 감사 로그 서명 키 |
| `NARAECLAW_LOCALE` | — | 로케일 (NaraeClaw 전용) |
| `NARAECLAW_BIN` | — | 바이너리 경로 (NaraeClaw 전용) |

## 탐색 순서

```
NARAECLAW_* → ZEROCLAW_* → 기본값
```

`NARAECLAW_*`가 설정되어 있으면 `ZEROCLAW_*`는 무시됩니다.

## 마이그레이션 권장 사항

- 새 설치: `NARAECLAW_*` 변수만 사용하세요.
- 기존 ZeroClaw 사용자: 기존 `ZEROCLAW_*` 변수가 그대로 동작합니다.
  시간이 될 때 `NARAECLAW_*`로 전환하세요.
- `ZEROCLAW_*` fallback은 향후 릴리즈에서 제거될 수 있습니다.

## 설정 파일 경로

| 항목 | 경로 |
|---|---|
| 기본 설정 디렉토리 | `~/.naraeclaw/` |
| 설정 파일 | `~/.naraeclaw/config.toml` |
| 워크스페이스 | `~/.naraeclaw/workspace/` |

ZeroClaw의 `~/.zeroclaw/` 경로는 자동으로 마이그레이션되지 않습니다.
기존 설정을 수동으로 복사하세요:

```bash
cp -r ~/.zeroclaw/* ~/.naraeclaw/
```
