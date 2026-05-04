# NaraeClaw Desktop V3 — 올인원 데스크탑 앱

> 목표: 컴맹도 더블클릭 한 번으로 AI 에이전트를 쓸 수 있게 한다.
>
> 업데이트: 2026-04-22

---

## 핵심 원칙

- **터미널 불필요** — 앱 안에서 모든 설정, 설치, 관리가 완료된다.
- **두 개의 바이너리** — `naraeclaw` (CLI, 서버용)와 `naraeclaw-desktop` (GUI, 개인용)은 독립적이다.
- **Desktop이 CLI를 포함** — 앱 번들 안에 `naraeclaw` 바이너리를 내장하고, sidecar로 관리한다.
- **첫 실행 = 온보딩** — 설정 없이 앱을 열면 앱 안에서 설정을 안내한다.

---

## 바이너리 구조

```
naraeclaw-desktop.app
├── NaraeClaw (Tauri GUI 바이너리)
├── naraeclaw (내장 CLI 바이너리, sidecar)
└── web/ (React 프론트엔드, 번들)
```

- `naraeclaw` CLI는 별도로 서버에 설치해서 쓸 수 있다.
- Desktop 앱은 CLI 없이도 독립 실행 가능하다 (내장 바이너리 사용).
- 향후 git 저장소 분리 가능 (현재는 같은 repo의 `apps/tauri/`).

---

## 기능 목록

### Phase 1 — 기본 동작 (MVP)

#### F1. 원클릭 시작
- 앱 더블클릭 → 모든 것이 자동으로 시작
- sidecar로 `naraeclaw gateway start` 자동 실행
- gateway healthy 확인 후 WebView 표시
- auto-pair (localhost에서 코드 입력 없이 토큰 자동 발급)
- 토큰 영속화 (Tauri store, 앱 재시작 시 재사용)

#### F2. 앱 내 온보딩
- `~/.naraeclaw/config.toml` 없으면 온보딩 화면 표시
- Provider 선택 (Ollama, OpenRouter, Anthropic, OpenAI 등)
- API key 입력 (클라우드 provider 선택 시)
- 모델 선택
- 설정 저장 → gateway 자동 재시작
- 온보딩 완료 후 바로 채팅 화면으로 전환

#### F3. Ollama 통합
- Ollama 설치 여부 자동 감지
- 미설치 시 → 앱 안에서 설치 안내 + 원클릭 설치 (macOS: brew, Linux: curl)
- 설치된 모델 목록 표시
- 모델 다운로드 (앱 안에서 `ollama pull` 실행, 진행률 표시)
- Ollama 서버 자동 시작/관리
- **모델 로드 실패 감지 → "다시 다운로드할까요?" 자동 제안**
- **모델 파일 손상 감지 → 자동 삭제 + 재다운로드**
- **Ollama 서버 비정상 종료 → 자동 재시작**

#### F3-1. 자가 치유 (Self-Healing)
- **모델 에러**: 채팅 중 모델 로드 실패 → 앱 내 알림 + "모델 재설치" 버튼
- **Ollama 서버 죽음**: health check 실패 → 자동 `ollama serve` 재시작
- **API key 만료/잘못됨**: 401/403 응답 → "API key를 확인하세요" + 설정 화면으로 이동
- **Gateway 비정상 종료**: sidecar 프로세스 죽음 감지 → 자동 재시작
- **네트워크 끊김**: 연결 실패 → 상태바에 "오프라인" 표시 + 자동 재연결 시도
- **디스크 부족**: 모델 다운로드 전 용량 체크 → 부족하면 경고
- **포트 충돌**: gateway 시작 실패 (Address in use) → 다른 포트 자동 시도 또는 기존 프로세스 종료 제안
- 모든 자가 치유 동작은 앱 내 알림으로 사용자에게 표시
- 자동 복구 실패 시 → 명확한 한국어 에러 메시지 + 해결 방법 안내

#### F4. 에이전트 채팅
- WebSocket 기반 실시간 채팅 UI
- 마크다운 렌더링
- 코드 블록 하이라이팅
- 도구 실행 결과 표시 (shell, file, http 등)
- 채팅 히스토리 유지

#### F5. 시스템 트레이
- 앱 창을 닫아도 트레이에 상주
- 트레이 아이콘 상태 표시 (대기/작업 중/오류/연결 끊김)
- 트레이 메뉴: 대시보드 열기, 에이전트 채팅, 상태, 종료
- 한국어 메뉴 텍스트

#### F6. 알림
- 에이전트 작업 완료 시 시스템 알림
- 채널 메시지 수신 시 알림
- 에러/경고 알림
- 알림 클릭 → 해당 화면으로 이동

### Phase 2 — 채널 & 연동

#### F15. 외부 AI CLI 도구 통합
- **지원 도구**: Claude Code, Codex CLI, Gemini CLI, Kiro CLI, OpenCode CLI
- **자동 감지**: `which claude`, `which codex` 등으로 설치 여부 확인
- **설치 관리**: 미설치 시 앱에서 설치 안내 또는 원클릭 설치
- **설정 UI**: 도구별 활성화/비활성화 토글, 기본 도구 선택
- **작업 위임**: 에이전트가 작업 종류에 따라 적절한 CLI에 자동 위임
  - 코드 리뷰/수정 → Claude Code 또는 Codex
  - 이미지/멀티모달 → Gemini CLI
  - 일반 대화 → 기본 provider
- **결과 통합**: 어떤 CLI를 썼든 앱 채팅 UI에서 통합 표시
- **`@` 멘션 지원**: 채팅에서 `@claude`, `@gemini`, `@kiro` 등으로 특정 도구 직접 지정
- **세션 공유**: NaraeClaw 메모리/컨텍스트를 외부 CLI에 전달 가능

#### F7. 채널 연동 설정
- 텔레그램 봇 토큰 설정 (앱 안에서)
- 슬랙 앱 토큰 설정
- 채널 활성화/비활성화 토글
- 채널 상태 모니터링 (연결됨/끊김)
- 채널별 메시지 로그 확인

**TODO — 채널 실제 연결:**
- [ ] sidecar를 `gateway start` → `daemon`으로 변경 (또는 gateway에 채널 시작 포함)
- [ ] 채널 저장 후 gateway 재시작 시 채널이 실제로 연결되는지 확인
- [ ] 채널 연결 실패 시 에러 메시지를 앱에서 표시
- [ ] 채널별 연결 상태 실시간 폴링

#### F17. 통합 서비스 관리
- 카탈로그가 아닌 실제 연결/관리 UI
- **연결됨**: 활성 서비스 목록, 설정 변경, 끄기/켜기 토글
- **연결 가능**: 원클릭 연결 → 앱 안에서 토큰/API key 입력 → 테스트 연결 → 활성화
- **준비 중**: 곧 출시 서비스 표시 (설정 불가)
- 지원 서비스:
  - 메신저: Telegram, Slack, Discord, Matrix
  - 개발: GitHub, GitLab, Jira
  - 생산성: Notion, Google Calendar, Google Workspace
  - AI: Ollama, Claude Code, Codex, Gemini
- 서비스별 연결 상태 실시간 표시 (✅ 연결됨 / ⚠️ 오류 / ⏸ 비활성)
- 연결 실패 시 → 에러 원인 + 해결 방법 안내
- 서비스별 사용량/로그 확인

#### F8. 파일 드롭
- 앱 창에 파일 드래그앤드롭 → 에이전트에 전달
- 이미지: 비전 모델로 분석
- 문서 (PDF, TXT, MD): 내용 추출 후 에이전트 컨텍스트에 추가
- 코드 파일: 코드 리뷰/분석 요청

#### F16. 자연어 예약 작업
- cron 문법 대신 자연어로 예약: "매일 아침 9시에 서버 상태 알려줘"
- 앱 UI에서 예약 작업 목록 관리 (추가/수정/삭제/일시정지)
- 반복 주기 시각적 선택: 매일/매주/매월 + 시간 피커
- 예약 작업 실행 결과를 알림으로 전달
- 실행 히스토리 확인 (성공/실패/결과 요약)
- 채팅에서 바로 등록: "이거 매주 월요일마다 해줘" → 자동 예약
- 내부적으로 cron 스케줄러로 변환하되, 사용자에게는 cron 문법 노출 안 함

#### F9. 클립보드 연동
- 클립보드 내용을 에이전트에 빠르게 전달 (단축키)
- 에이전트 응답을 클립보드에 복사
- 스크린샷 → 클립보드 → 에이전트 분석

### Phase 3 — 고급 기능

#### F10. 컴퓨터 제어 (Computer Use)
- 에이전트가 마우스/키보드 제어
- 스크린샷 기반 화면 인식
- 브라우저 자동화
- 권한 관리 (사용자 승인 필요)
- 안전 장치 (긴급 정지, 작업 범위 제한)

#### F14. 내 컴퓨터 리소스 활용 ("또 하나의 나")
- **웹 브라우저**: 에이전트가 브라우저를 열어 검색, 로그인된 서비스 조작 (Gmail, 캘린더, 노션 등)
- **파일 시스템**: 파일 읽기/쓰기/정리, 폴더 구조 파악, 문서 요약
- **앱 제어**: 다른 앱 실행/조작 (Finder, 터미널, 메모, 미리보기 등)
- **시스템 정보**: CPU/메모리/디스크 상태, 프로세스 목록, 네트워크 상태
- **스케줄링**: "매일 아침 9시에 이메일 요약해줘" 같은 반복 작업
- **멀티태스킹**: 사용자가 다른 일 하는 동안 백그라운드에서 작업 수행
- **컨텍스트 인식**: 현재 열린 앱/문서/탭을 파악하고 상황에 맞는 도움 제공
- 모든 리소스 접근은 사용자 권한 정책으로 제어 (autonomy level)

#### F11. 원격 NaraeClaw 연결
- 원격 서버의 `naraeclaw gateway`에 연결
- SSH 터널 또는 직접 연결
- 서버 목록 관리 (추가/제거/전환)
- 로컬 ↔ 원격 전환
- 원격 서버 상태 모니터링

#### F12. 지식 관리 (위키 + 메모리 통합)
- 마크다운 위키 에디터 — 에이전트의 장기 기억 저장소
- 에이전트가 위키를 참조하여 답변
- 위키 페이지 생성/편집/삭제/검색
- 태그/카테고리 분류
- 채팅에서 "이거 기억해" → 자동으로 위키에 저장
- 대화 메모리(자동 저장)와 위키(수동 정리)를 하나의 UI에서 관리
- 메모리 → 위키 승격: 자동 저장된 대화 메모리를 위키 페이지로 정리
- 검색: 위키 + 대화 메모리 통합 검색
- 임포트/익스포트 (마크다운 파일)
- 사용량 통계

**TODO — 기존 knowledge_graph 시스템 통합:**
- [ ] 위키 UI를 `knowledge_graph.rs` + `knowledge_tool.rs` 기반으로 전환
- [ ] `[knowledge] enabled = true`를 온보딩 기본값으로 설정
- [ ] 위키 페이지를 knowledge graph 노드로 저장 (Pattern/Decision/Lesson 타입)
- [ ] 에이전트 시스템 프롬프트에 관련 knowledge 자동 주입 (`suggest_on_query`)
- [ ] 대화 중 "이거 기억해" → knowledge capture 자동 호출
- [ ] 위키 UI에서 노드 간 관계(uses/extends/replaces) 시각화
- [ ] 시맨틱 검색 (임베딩 기반)
- [ ] `auto_capture: true` 시 대화에서 자동 지식 추출

---

## 기술 스택

| 레이어 | 기술 |
|---|---|
| GUI 프레임워크 | Tauri 2.0 (Rust backend + WebView) |
| 프론트엔드 | React 19 + TypeScript + Tailwind CSS |
| 백엔드 (sidecar) | `naraeclaw` CLI 바이너리 (`gateway start`) |
| IPC | Tauri Commands (Rust ↔ JS) |
| 상태 관리 | Tauri Store (영속화) + React Context |
| 알림 | tauri-plugin-notification |
| 트레이 | Tauri tray-icon (이미 구현) |
| 파일 드롭 | Tauri drag-drop event |
| 클립보드 | tauri-plugin-clipboard |
| 자동 업데이트 | tauri-plugin-updater |

---

## 앱 실행 흐름

```
앱 시작
  │
  ├─ config.toml 존재?
  │   ├─ NO → 온보딩 화면
  │   │        ├─ Ollama 감지 → 설치/모델 다운로드
  │   │        ├─ Provider 선택 + API key
  │   │        ├─ config.toml 저장
  │   │        └─ → gateway 시작으로 이동
  │   │
  │   └─ YES → gateway 시작
  │              ├─ sidecar: naraeclaw gateway start
  │              ├─ health poll (최대 30초)
  │              ├─ auto-pair (토큰 자동 발급)
  │              ├─ 토큰 store 저장
  │              └─ WebView에 대시보드 표시
  │
  └─ 시스템 트레이 등록
       ├─ 창 닫기 → 트레이 상주
       ├─ 트레이 메뉴 → 대시보드/채팅/종료
       └─ 앱 종료 → sidecar SIGTERM → SIGKILL
```

---

## 온보딩 화면 흐름

```
1. 환영 화면
   "NaraeClaw — 서버 관리와 개인 지식을 위한 AI 에이전트"
   [시작하기]

2. AI 엔진 선택
   ○ Ollama (로컬, 무료, 추천)
   ○ OpenRouter (클라우드, 200+ 모델)
   ○ Anthropic (Claude)
   ○ OpenAI (GPT)
   ○ 기타

3-A. Ollama 선택 시
   ├─ Ollama 감지
   │   ├─ 설치됨 → 모델 선택
   │   └─ 미설치 → [Ollama 설치하기] 버튼
   │              └─ 설치 진행 (진행률 표시)
   ├─ 모델 선택/다운로드
   │   ├─ 추천: gemma3, llama3.2, qwen2.5
   │   └─ [다운로드] → 진행률 표시
   └─ 완료

3-B. 클라우드 provider 선택 시
   ├─ API key 입력
   ├─ 모델 선택
   └─ 완료

4. 완료
   config.toml 저장 → 대시보드로 전환
```

---

## Phase 구분 및 우선순위

| Phase | 기능 | 우선순위 | 의존성 |
|---|---|---|---|
| 1 | F1 원클릭 시작 | P0 | — |
| 1 | F2 앱 내 온보딩 | P0 | F1 |
| 1 | F3 Ollama 통합 | P0 | F2 |
| 1 | F3-1 자가 치유 | P0 | F3 |
| 1 | F4 에이전트 채팅 | P0 | F1 (이미 있음) |
| 1 | F5 시스템 트레이 | P0 | F1 (이미 있음) |
| 1 | F6 알림 | P1 | F5 |
| 2 | F15 외부 AI CLI 통합 | P1 | F4 |
| 2 | F7 채널 연동 | P1 | F2 |
| 2 | F17 통합 서비스 관리 | P1 | F7 |
| 2 | F8 파일 드롭 | P1 | F4 |
| 2 | F16 자연어 예약 작업 | P1 | F4 |
| 2 | F9 클립보드 | P1 | F4 |
| 3 | F10 컴퓨터 제어 | P2 | F4 |
| 3 | F14 컴퓨터 리소스 활용 | P2 | F10 |
| 3 | F11 원격 연결 | P2 | F1 |
| 3 | F12 지식 관리 (위키+메모리) | P1 | F4 |

---

## 현재 상태 (2026-04-22)

이미 구현된 것:
- [x] Tauri 2.0 앱 기본 구조
- [x] sidecar 관리 (spawn/shutdown, graceful SIGTERM)
- [x] 시스템 트레이 (한국어 메뉴, 상태 아이콘)
- [x] auto-pair (3회 재시도, store 영속화)
- [x] health polling + 트레이 아이콘 업데이트
- [x] WebView token inject + 리로드
- [x] 창 크기/위치 저장/복원
- [x] 싱글 인스턴스

아직 없는 것:
- [ ] 앱 내 온보딩 UI (React 페이지)
- [ ] Ollama 설치/모델 관리
- [ ] 채널 연동 설정 UI
- [ ] 파일 드롭
- [ ] 클립보드 연동
- [ ] 시스템 알림
- [ ] 컴퓨터 제어
- [ ] 원격 연결
- [ ] 메모리 관리 UI (웹에 기본 있음, 개선 필요)
- [ ] 위키 지식 관리

---

## 비고

- CLI (`naraeclaw`)는 현재 repo에서 계속 유지. Desktop은 `apps/tauri/`에서 개발.
- git 분리는 Desktop이 안정화된 후 검토.
- macOS 우선 개발, Windows/Linux는 Phase 1 완료 후.
- Ollama 설치는 OS별 분기 필요 (macOS: brew/dmg, Linux: curl script).
