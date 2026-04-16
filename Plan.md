# NaraeClaw 작업 계획

> 최종 목표: ZeroClaw 포크를 **텔레그램 중심·한국어 우선·경량·안전** 에이전트로 전환한다.
> 업데이트: 2026-04-16 (핵심 아키텍처 완료, 데스크탑 V2 진행 중)

---

## 전체 우선순위 요약

| # | 작업 | 위험도 | 상태 | 담당 |
|---|------|--------|------|------|
| ✅ 1 | unsafe 환경 변수 → OnceLock 전환 | 중위험 | **완료** | Claude |
| ✅ 2 | Default feature 경량화 | 중위험 | **완료** | Codex |
| ✅ 3 | 하드코딩 설정 유연화 (Provider URL) | 중위험 | **완료** | Claude |
| ✅ 4 | Telegram polling → webhook 전환 | 고위험 | **완료** | Codex |
| ✅ 5 | zeroize 패턴 도입 | 중위험 | **완료** | Claude |
| ✅ 6 | 보안 메커니즘 통합 (CredentialFilter) | 고위험 | **완료** | Codex |

---

## 작업 완료 리포트

### **1. 안정성 및 동시성 개선**
- **내용**: 프로덕션 코드 내 `unsafe { set_var }` 제거 및 `OnceLock`, `Mutex` 기반의 안전한 상태 관리로 전환.
- **결과**: 비동기 런타임에서의 레이스 컨디션 및 정의되지 않은 동작(UB) 원천 차단.

### **2. 빌드 최적화 (Feature-gate)**
- **내용**: 24개 이상의 채널을 카테고리별(CN, Social, Legacy)로 분리.
- **결과**: 바이너리 크기 대폭 감소, 불필요한 의존성 제거로 보안 공격 표면 축소.

### **3. 텔레그램 웹훅(Webhook) 지원**
- **내용**: 기존 Polling 방식 외에 고성능 Webhook 모드 추가 (Axum 기반).
- **결과**: 즉각적인 응답 속도 확보 및 서버 리소스 효율화.

### **4. 통합 보안 엔진 (CredentialFilter)**
- **내용**: 파편화되어 있던 유출 탐지 로직을 `security` 크레이트로 일원화.
- **기능**: Base64/Hex/URL-encoded 토큰 탐지, 스트리밍 청크 경계 탐지 지원.
- **결과**: 에이전트의 실수로 인한 API 키 유출 사고 방지 강화.

### **5. 데이터 보호 (SecretStore & Zeroize)**
- **내용**: `SecretStore` 키 암호화 유지 및 메모리 내 민감 데이터 `zeroize` 처리.
- **결과**: 파일 및 메모리 덤프를 통한 정보 유출 방어.

---

## V2 — 데스크탑 앱 포팅

> 목표: CLI + 브라우저 조합 → **NaraeClaw 단일 데스크탑 앱** (macOS 우선)
>
> 현황: Tauri 2.0 앱(`apps/tauri/`), React 19 프론트엔드(`/web/src/`),
> Axum Gateway(`zeroclaw-gateway`)가 이미 골격 구현돼 있음.
> Gateway가 에이전트 런타임을 인프로세스로 임베드하고,
> Tauri 앱은 WebView로 Gateway의 웹 UI를 렌더링하는 구조.

### 아키텍처 현황

```
현재:  사용자 ──▶ Tauri 트레이 앱 ──HTTP/WS──▶ Gateway (별도 프로세스)
                  (WebView)                      Axum + 에이전트 런타임

목표:  사용자 ──▶ NaraeClaw 앱 (단일 프로세스)
                  ├─ Tauri 창 (WebView)
                  └─ Gateway sidecar (자동 시작/종료)
```

### 작업 목록

| # | 작업 | 위험도 | 상태 | 담당 |
|---|------|--------|------|------|
| ✅ A | **Gateway sidecar 번들링** — `naraeclaw agent` 실행파일을 Tauri sidecar로 묶어 앱 실행 시 자동 시작 | 중 | **완료** | Claude |
| ✅ B | **창 기본 표시** — `visible: false` → `true`, 창 크기·위치 저장 (tauri-plugin-store) | 하 | **완료** | Claude |
| ◐ C | **한국어 UI** — 웹 프론트엔드(`/web/src/`) 메뉴·버튼·안내 텍스트 한국어화 | 하 | **진행 중** | Codex |
| D | **시스템 알림** — Telegram 메시지 수신 시 native notification (tauri-plugin-notification) | 하 | 미착수 | - |
| ◐ E | **앱 브랜딩** — 앱 이름 NaraeClaw, 아이콘 교체, About 창 | 하 | **부분 완료** | Claude/Codex |
| F | **파일 드롭·클립보드** — Tauri plugin으로 첨부파일 전달 지원 | 하 | 미착수 | - |

### 작업 우선순위

1. **A (sidecar 번들링)** — 이게 없으면 나머지 개선이 의미 없음. 핵심 선결 조건.
2. **C (한국어 UI)** — NaraeClaw 정체성. 기본 언어를 한국어로 전환하고, 남은 사용자 노출 영어/ZeroClaw 문구를 정리.
3. **E (브랜딩 마무리)** — About 창과 남은 웹 UI 브랜딩 잔재 정리.
4. **D + F (알림·파일)** — 편의 기능, 이후 단계.

### 참고 파일 경로

| 구성 요소 | 경로 |
|---|---|
| Tauri 앱 진입점 | `apps/tauri/src/lib.rs` |
| 창·권한 설정 | `apps/tauri/tauri.conf.json` |
| Gateway 클라이언트 | `apps/tauri/src/gateway_client.rs` |
| Gateway 라우트 | `crates/zeroclaw-gateway/src/lib.rs` |
| WebSocket 채팅 | `crates/zeroclaw-gateway/src/ws.rs` |
| React 앱 진입점 | `web/src/App.tsx` |
| 채팅 페이지 | `web/src/pages/AgentChat.tsx` |

---

## 향후 과제 (추가)
- [ ] 사용자 정의 스킬(Custom Skill) 한국어 템플릿 확충
- [ ] 로컬 LLM(Ollama 등) 연동 가이드 문서화
- [ ] 멀티 모달(이미지 분석) 기능의 한국어 프롬프트 최적화
