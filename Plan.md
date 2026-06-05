# NaraeClaw Maintainer Plan

> 목표: NaraeClaw를 **한국어 우선, 가볍고 단단한 서버 관리 + 개인 지식 관리 에이전트**로 정리한다.
>
> 현재 단계: 큰 실험형 포크를 실제 유지 가능한 제품형 코드베이스로 줄이는 중.
> 새 기능보다 **핵심 경로 안정화, 제품 정체성, 단순한 검증 루프**를 우선한다.
>
> 업데이트: 2026-04-22

---

## Product Direction

NaraeClaw는 다음 사용자를 우선한다.

- 개인 서버, VPS, 홈서버, 클라우드 인프라를 관리하는 사용자
- CLI와 게이트웨이 API로 에이전트를 제어하려는 사용자
- 개인 지식 관리, 로그 요약, 반복 운영 작업 자동화를 원하는 사용자
- 한국어 기본 경험을 기대하는 사용자

명시적으로 우선하지 않는 영역:

- 로봇, 임베디드 보드, 펌웨어, 특수 디바이스
- 중국 지역 전용 채널 또는 지역 한정 메신저
- 유지 부담이 큰 marketplace/distribution 자동화
- 아직 실제 사용자 흐름이 없는 플러그인 생태계
- “언젠가 쓸 수도 있는” 넓은 통합 목록

Desktop과 Web은 제거되었다 (2026-05-05). CLI + 게이트웨이 API가 유일한 제품 표면이다.

---

## Current Maintainer Posture

지금은 fast development mode다.

- 작은 owner-approved 변경은 `master` 직접 커밋을 허용한다.
- CI는 빠르게 통과 가능한 핵심 검증만 유지한다.
- 기능 추가보다 삭제, 정리, 첫 실행 경험 개선을 우선한다.
- PR은 큰 변경, 보안/런타임 위험 변경, 외부 리뷰가 필요한 경우에 사용한다.
- 모든 변경은 작고 되돌리기 쉬워야 한다.

권장 기본 검증:

```bash
cargo fmt --all -- --check
cargo check --workspace
```

변경 범위에 따라 추가:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p naraeclaw-config --lib
cargo test -p naraeclaw-runtime --lib
cargo test -p naraeclaw-gateway --lib --no-run
cargo test -p naraeclaw-tui --lib
```

---

## Priority Roadmap

| Priority | Workstream | Goal | Status |
|---|---|---|---|
| P0 | 제품 정체성 정리 | README, AGENTS, 온보딩, 사용자 노출 문구가 같은 제품을 설명하게 만든다 | 완료 |
| P0 | 핵심 경로 안정화 | `onboard -> configure -> agent/gateway -> tool use` 흐름을 깨지 않게 만든다 | 완료 |
| P0 | 검증 루프 단순화 | 빠른 개발을 막는 과도한 CI/CD와 낡은 체크를 제거한다 | 완료 |
| P1 | stale scope 제거 | 로봇/디바이스/marketplace/CN/legacy channel/plugin 잔재를 제거한다 | 완료 |
| P1 | Desktop/Web 제거 | `apps/tauri/`와 `web/` 삭제 — CLI + 게이트웨이 API로 단일화 | 완료 (2026-05-05) |
| P1 | 설정/온보딩 정리 | NaraeClaw 기본값, 경로, 문구, provider 설정을 단순화한다 | 완료 |
| P2 | 릴리즈 경로 재정의 | macOS/Windows/Linux 설치와 배포만 남긴다 | 완료 (workflow 구현은 릴리즈 케이던스 후) |
| P2 | 테스트 부채 축소 | 오래된 통합/컴포넌트 테스트를 핵심 경로 중심으로 재분류한다 | 완료 (채널 제거 잔재 정리). Follow-up: naraeclaw-channels 프롬프트 어셈블리 테스트 12개 drift 별도 작업 |

---

## P0 — Product Identity Cleanup

목표: 새 사용자가 README와 온보딩만 보고도 “이 프로젝트가 무엇인지” 이해하게 한다.

작업:

- [x] README를 서버 관리 + 개인 지식 관리 + CLI/Desktop/Web 중심으로 다시 정리
- [x] “ZeroClaw 포크”라는 출처는 명확히 남기되, 현재 제품명은 NaraeClaw로 일관화
- [x] AGENTS.md에서 삭제된 plugin/hardware/device-era guidance 제거
- [x] README, docs, onboarding의 사용자 노출 문구에서 남은 Zeroclaw/ZeroClaw 흔적 점검
- [x] `naraeclaw onboard` TUI 첫 화면 문구 수정 ("the fastest, smallest AI assistant" → 서버관리+개인지식 에이전트 방향)

완료 기준:

- README의 첫 1분 경험이 명확하다. ✅
- AGENTS.md가 현재 유지보수 정책과 충돌하지 않는다. ✅
- 사용자에게 보이는 기본 문구가 NaraeClaw로 일관된다. ✅

---

## P0 — Core Path Stabilization

목표: 핵심 사용 흐름을 릴리즈마다 반드시 살린다.

핵심 경로:

1. `naraeclaw onboard`
2. provider/model/API key 설정
3. workspace/config 저장
4. `naraeclaw agent` 실행
5. shell/file/http/browser/memory 도구 사용
6. gateway API 접근 (포트 42617)

작업:

- [x] 온보딩 기본값을 한국어 우선, 개인 서버 관리용으로 조정
- [x] CLI help와 error message에서 오래된 제품명/범위 제거
- [x] config 저장/로드 테스트를 핵심 경로 기준으로 유지
- [x] gateway health/session/chat 기본 경로 점검

완료 기준:

- 새 checkout에서 최소 설정 후 agent/gateway가 실행된다. ✅
- CLI, gateway 중 하나를 고쳐도 나머지 핵심 경로가 깨지지 않는다. ✅

---

## P0 — Simple CI/CD

목표: 빠른 개발을 방해하는 과거 CI/CD 매트릭스를 제거하고, 항상 실행 가능한 검증만 남긴다.

기본 CI:

- format
- workspace check excluding desktop
- targeted clippy or tests for changed surfaces

제외하거나 별도 수동 작업으로 내릴 대상:

- 유지하지 않는 marketplace 배포
- AUR/Dokploy/EasyPanel 등 현재 제품 범위 밖 배포
- live/manual 테스트
- 특수 OS/디바이스/보드 전용 검증

작업:

- [x] `.github/workflows/`가 현재 제품 범위와 맞는지 재점검
- [x] `dev/ci.sh`, `Justfile`, docs의 검증 명령을 같은 기준으로 통일
- [x] `--locked`가 Cargo.lock 갱신과 충돌하는 흐름 정리 (Justfile에서 제거, ci.sh Docker는 유지)
- [x] desktop 검증은 별도 job 또는 수동 검증으로 분리

완료 기준:

- CI 실패가 실제 회귀를 의미한다. ✅
- 빠른 변경이 낡은 배포/문서/marketplace 체크에 막히지 않는다. ✅

---

## P1 — Remove Stale Scope

최근 제거 완료:

- CN/regional China 채널
- marketplace, firmware, dist 일부
- live/manual 테스트
- WASM plugin crate/API
- IRC/iMessage/Linq legacy channels

남은 점검 대상:

- [x] docs에 남은 plugin ecosystem, hardware, peripherals, boards, robot 표현
- [x] CI/CD에 남은 marketplace/distribution automation
- [x] install/release script에 남은 비지원 플랫폼 가정
- [x] config schema `PluginsConfig`에 `#[deprecated]` 표시, 제거 예정 명시
- [x] README와 docs의 과장된 multi-platform/device claim

삭제 원칙:

- macOS/Windows/Linux desktop/server에서 쓸 일이 없으면 삭제 후보
- Desktop/Web 개인 편의성은 유지
- 서버 관리와 개인 지식 관리에 직접 연결되지 않으면 낮은 우선순위
- 단순 compatibility fallback은 유지할 수 있지만 사용자 노출은 줄인다

---

## P1 — Desktop and Web (제거 완료)

**2026-05-05 결정: `apps/tauri/`와 `web/` 전체 삭제.**

바이너리 크기와 유지 부담을 줄이고 CLI + 게이트웨이 API로 단일화한다.
릴리즈 산출물: `naraeclaw` 단일 바이너리 (~11MB, release 프로파일 기준).

유지되는 표면:
- `naraeclaw agent` — 대화형 CLI
- `naraeclaw gateway` — HTTP/WebSocket API (포트 42617)

---

## P1 — Configuration and Onboarding

목표: 설정이 강력하되 첫 사용자가 압도되지 않게 한다.

작업:

- [x] 기본 config를 핵심 기능 중심으로 줄인다 (이미 serde(default)로 최소 config 동작 확인)
- [x] 오래된 provider/channel/tool 설정 주석 제거 (삭제된 채널 잔재 없음 확인)
- [x] `Config` 테스트 helper/builder 도입 검토 → 현재 패턴(Config::default() + 필드 수정)으로 충분, builder 불필요
- [x] secret/memory/gateway 기본 경로를 NaraeClaw 이름으로 통일
- [x] compatibility fallback 문서화 → `docs/ops/compatibility-fallback.md`

완료 기준:

- `Config` 변경이 테스트 전체를 쉽게 깨뜨리지 않는다.
- 온보딩 선택지가 실제 유지 기능만 보여준다.

---

## P2 — Release and Packaging

목표: macOS/Windows/Linux만 명확히 지원한다.

작업:

- [x] install script가 현재 지원 OS만 설명하게 정리 (macOS/Linux 4 target 명시)
- [x] Docker 사용 여부를 서버 운영 목적에 맞게 재검토 → 유지 (서버 배포 핵심 경로)
- [x] package metadata에서 ZeroClaw/NaraeClaw 혼재 제거 (잔재 없음 확인)
- [x] release workflow를 CLI/Desktop/Web 산출물 기준으로 단순화 (workflows README에 계획 명시, 구현은 릴리즈 케이던스 확정 후)

완료 기준:

- 릴리즈 경로가 적고 예측 가능하다. ✅
- 지원하지 않는 OS/device 배포 흔적이 없다. ✅

---

## Near-Term Execution Order

P0–P1 핵심 작업은 모두 완료되었다. 남은 작업:

1. naraeclaw-channels 프롬프트 어셈블리 테스트 12개 drift 수정 (P2, 채널 제거 잔재 정리 follow-up)
2. Release workflow 실제 구현 — CLI 단일 바이너리 기준으로 단순화, 릴리즈 케이던스 확정 후 (P2)

### ADR-005 자동 스킬 진화 루프 (`docs/architecture/adr-005-auto-skill-evolution.md`)

M1·M2·M2.5·M4 완료. M3a 핫 리로드·M3b Memory 브리지·Level 2 도구 교체 완료(2026-06-05). channels clippy 부채 정리(2026-06-05). 남은 작업:

3. **M3b 실시간 trigger 연결** — consolidation의 `skill_candidate` 마킹을 다음 턴 `try_trigger`의 `user_signal`로 잇기. `consolidate_turn`이 fire-and-forget이라 타이밍 설계(마킹 소비 규칙) 필요 (중위험)
4. **M5 포맷 호환 레이어** — Hermes import/export (보류, 외부 마켓 연동 후속)

---

## Non-Goals For Now

- 새 채널 대량 추가
- 플러그인 생태계 재도입
- 로봇/하드웨어/펌웨어 지원
- 모든 과거 upstream feature parity 유지
- 복잡한 릴리즈 매트릭스 복구
- 외부 기여자용 대형 governance 문서 확장
