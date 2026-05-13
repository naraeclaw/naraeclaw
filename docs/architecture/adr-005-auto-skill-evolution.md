# ADR-005: Auto Skill Evolution Loop

**Status:** Accepted

**Date:** 2026-05-13 (proposed), 2026-05-13 (accepted, decisions finalized)

**Related modules:** `naraeclaw-runtime/src/{skills,skillforge}`, `naraeclaw-memory`, `naraeclaw-runtime/src/agent`

**Inspiration:** [Hermes Agent](https://hermes-agent.org/) (Nous Research, 2026-02)

## Context

Hermes Agent가 강조하는 **자동 스킬 진화 루프**를 NaraeClaw에 도입한다. 현재 코드베이스는 `SkillCreator`, `SkillImprover`, `skillforge`, 정교한 메모리 시스템(LRU+FTS+Vector, namespace, consolidation, decay) 등 *인프라는 모두 갖췄지만*, 이들을 자동으로 엮어 도는 루프가 없어 스킬이 한 건도 생성되지 않는다. 본 ADR은 이미 보유한 컴포넌트를 외부 의존성 추가 없이 엮어 자동 진화 루프를 구현하는 설계를 기술한다.

## Decision

7개 비트(B1~B7)로 구성된 자동 진화 루프를 5단계 마일스톤(M1~M5)으로 도입한다. 각 단계는 별도 worktree에서 진행하고, config 기본값은 off로 출시해 안정화 후 on.

**확정된 정책 결정** (구버전 Section 8 오픈 질문에서 전환):

| # | 결정 | 비고 |
|---|---|---|
| D1 | 트리거 임계값 기본값 **0.6** | ValueSignal::score >= 0.6에서 자동 생성 |
| D2 | 사용자 명시 신호 **키워드 + 도구 둘 다** | "기억해줘" 등 키워드는 consolidation LLM이 추출, `mark_skill_candidate` 도구도 별도 제공 |
| D3 | 스킬 인덱스 카테고리 **`Custom("skill_index")` 신설** | 검색 시 카테고리 필터·decay 정책 독립 조절 |
| D4 | 폐기 정책 **decay 기반 자연 폐기** | 별도 archive 명령 없음. 파일은 유지(복원 가능) |
| D5 | 스킬 포맷 **양쪽 동시 지원 (변환 레이어)** | 내부 저장은 NaraeClaw 고유, 외부 익스포트/임포트 시 Hermes 포맷 변환. M5 신설 |
| D6 | consolidation LLM 비용 **+10–15% 허용** | `skill_candidate` 필드를 기존 consolidation 추출에 통합 (별도 호출 분리 안 함) |

상세 설계는 아래 본문 참조.

---

## 1. 목표

> "어려운 문제를 해결하면, NaraeClaw가 그 해결책을 *스스로* 재사용 가능한 스킬로 만들고, 다음에는 그 스킬을 *자동으로* 끌어다 쓴다."

비목표(out of scope):
- 새 채널 추가 (Telegram·Discord 등) — 별도 트랙
- 외부 스킬 마켓 통합 (agentskills.io 디렉토리 업로드·검색) — 본 ADR은 *포맷 변환*까지만, 마켓 연동은 후속

---

## 2. 배경 — Hermes Agent와의 비교

Hermes 공식 문서가 강조하는 4가지 특성을 NaraeClaw 현재 상태와 매핑.

| Hermes 특성 | NaraeClaw 현황 | 갭 |
|---|---|---|
| 영구 기억 | `naraeclaw-memory` 완비 (LRU+FTS+Vector, namespace, consolidation, decay, importance) | 없음 — 오히려 더 정교 |
| 자동 스킬 생성 | `skills/creator.rs`에 `SkillCreator::create_from_execution()` 구현 | **호출 지점 부재** — 자동 트리거 없음 |
| 멀티플랫폼 게이트웨이 | `channels/{cli,slack}` + `naraeclaw-gateway` | 채널 수 부족 (별도 트랙) |
| 스킬 검색·공유 | `skills/mod.rs::skills_to_prompt()`로 매 턴 LLM 컨텍스트 주입, `skillforge` GitHub 정찰 | 메모리 의미 검색과 미통합, 사용 통계 부재 |

**Hermes 공식 사이트에서 *드러내지 않은* 것** (WebFetch 결과): 트리거 임계값, SKILL.md 필드 구조, 검색 방식 내부 세부사항. 즉 우리는 외부 호환성을 따라가는 게 아니라 *행동 패턴*만 참고하고 내부 구현은 자유롭게 설계한다.

---

## 3. 현재 NaraeClaw 상태 (코드 근거)

### 3.1 스킬 인프라 (이미 있음)

- `skills/mod.rs:126-174` — `load_skills()`: 부팅 시 `~/.naraeclaw/workspace/skills/` 스캔, audit 통과만 로드
- `skills/mod.rs:757-874` — `skills_to_prompt()`: 매 턴 LLM 컨텍스트에 XML로 주입
- `skills/mod.rs:882-915` — `skills_to_tools()`: `Tool` 트레이트로 등록 (호출 가능)
- `skills/creator.rs:37-81` — `SkillCreator::create_from_execution(task_desc, tool_calls, embedding_provider)`: 멀티스텝 실행 → SKILL.toml/md 작성, 임베딩 기반 중복 제거, LRU 제한
- `skills/improver.rs:46-117` — `SkillImprover::improve_skill(slug, content, reason)`: 원자적 갱신, 쿨다운
- `skills/audit.rs:34-65` — 심볼릭 링크/위험 패턴/셸 체이닝 차단

### 3.2 메모리 인프라 (이미 있음, 매우 정교)

- `memory_traits.rs:30-47` — `MemoryEntry { id, key, content, category, namespace, importance, superseded_by, ... }`
- `retrieval.rs:24-43, 114-172` — `RetrievalPipeline`: LRU(256/300s) → FTS5(BM25, 0.85 조기종료) → Vector(코사인)
- `consolidation.rs:55-136` — `consolidate_turn()`: 매 턴 후 fire-and-forget으로 Daily(history_entry) + Core(memory_update) 추출
- `importance.rs:8-49` — 카테고리·키워드 휴리스틱
- `decay.rs:8-45` — 반감기 7일, Core는 evergreen
- `namespaced.rs:19-72` — 에이전트/프로젝트 격리

### 3.3 SkillForge (이미 있음, 외부 스킬 정찰)

- `skillforge/mod.rs:123-223` — `forge()`: scout → evaluate → integrate 직렬 실행
- `skillforge/scout.rs` — GitHub/ClawHub/HuggingFace 검색 (고정 쿼리 `"zeroclaw skill"`, `"ai agent skill"`)
- `skillforge/evaluate.rs` — 호환성(30%)+품질(35%)+보안(35%) 스코어, Auto/Manual/Skip 추천
- `skillforge/integrate.rs:30, 56-94` — `./skills/<safe>/SKILL.toml,SKILL.md` 직접 작성

---

## 4. 갭 분석

**한 줄 요약**: 인프라는 다 있는데 *연결선*이 없다.

| # | 갭 | 영향 |
|---|---|---|
| G1 | `SkillCreator::create_from_execution()` 호출 지점이 코드 어디에도 없음 | 자동 생성 0건 |
| G2 | `SkillImprover::improve_skill()` 호출 지점이 코드 어디에도 없음 | 개선 0건 |
| G3 | "성공한 멀티스텝"을 판정하는 신호 없음 (모든 턴을 스킬화하면 노이즈 폭증) | 트리거 설계 필요 |
| G4 | `load_skills()`는 부팅 시 1회만 — 새 스킬 즉시 반영 안 됨 | 같은 세션 내 재사용 불가 |
| G5 | `consolidation`의 `memory_update` 추출 프롬프트가 "스킬 후보" 신호를 모름 | 메모리↔스킬 브리지 부재 |
| G6 | `SkillForge::forge()`가 명시적 호출만 — `scan_interval_hours` 설정만 있고 스케줄러 없음 | 백그라운드 진화 부재 |
| G7 | 스킬 사용 통계 없음 (어떤 게 자주 쓰이는지, 어떤 게 실패하는지) | 개선·폐기 판단 불가 |

---

## 5. 설계 — 자동 진화 루프 7 비트

### 5.1 전체 데이터 흐름

```
┌──────────────────────────────────────────────────────────────────────┐
│                          Agent Loop (per turn)                       │
│                                                                      │
│  user msg → planner → tool_calls[] → response                        │
│                              │                                       │
│                              ▼                                       │
│                      [B1] ExecutionTrace 캡처                        │
│                              │                                       │
│                              ▼                                       │
│                      [B2] ValueSignal 평가                           │
│                              │                                       │
│                       ┌──────┴──────┐                                │
│                       │             │                                │
│                  ≥threshold      <threshold                          │
│                       │             │                                │
│                       ▼             ▼                                │
│             [B3] SkillTrigger    Memory만 적재                       │
│                       │                                              │
│                       ▼                                              │
│         creator.create_from_execution()                              │
│                       │                                              │
│                       ▼                                              │
│             [B4] HotReload(new_skill)                                │
│                       │                                              │
│                       ▼                                              │
│         다음 턴부터 prompt+tools에 반영                              │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
       ▲                                                ▲
       │                                                │
[B5] Memory Bridge                              [B6] Usage Stats
  consolidation이                                 어떤 스킬이 쓰였나,
  스킬 후보 감지                                  성공/실패율 누적
       │                                                │
       └──────────► improver / archiver ◄───────────────┘
                          │
                          ▼
              [B7] (별도) SkillForge 스케줄러
                  주기적 외부 스킬 정찰
```

### 5.2 Bit 1 — ExecutionTrace 캡처

**목표**: 한 턴의 도구 호출 시퀀스를 구조체로 모은다.

**위치**: `naraeclaw-runtime/src/agent/loop_.rs`의 턴 종료 직전.

```rust
// 의사 코드
pub struct ExecutionTrace {
    pub turn_id: String,
    pub user_message: String,
    pub tool_calls: Vec<ToolCallRecord>,  // (이미 creator.rs:291-370 에서 사용)
    pub assistant_response: String,
    pub started_at: DateTime,
    pub finished_at: DateTime,
    pub error_count: u32,
    pub retry_count: u32,
}
```

이미 `creator::extract_tool_calls_from_history()`가 비슷한 일을 하지만 *사후 추출*이다. 직접 수집하면 비용 절감 + 정확도 향상.

### 5.3 Bit 2 — ValueSignal 평가

**목표**: 이 trace가 스킬로 만들 *가치가 있는가*를 판정.

`SkillCreator`는 이미 "tool_calls.len() ≥ 2" + 임베딩 유사도 < threshold만 검사한다. 더 풍부한 신호가 필요하다.

```rust
pub struct ValueSignal {
    pub multistep: bool,           // tool_calls ≥ 2
    pub succeeded: bool,            // 마지막 tool_call 성공 + 응답에 실패 키워드 없음
    pub novelty: f64,               // 1 - max_cosine_similarity(기존 스킬 임베딩)
    pub friction: f64,              // retry_count / total_calls (높을수록 가치 ↑)
    pub user_signal: Option<UserSignal>,  // "기억해줘", "좋다", thumbs_up 등
}

pub fn score(s: &ValueSignal) -> f64 {
    if !s.multistep || !s.succeeded { return 0.0; }
    let base = 0.4 * s.novelty + 0.3 * s.friction.min(1.0) + 0.3;
    base + s.user_signal.map(|u| u.weight()).unwrap_or(0.0)
}
```

**임계값**: 기본 0.6 (config로 노출). 사용자 명시 신호("기억해줘")는 임계값 무조건 우회.

**탐지 휴리스틱**:
- `succeeded`: 마지막 tool result에 `error|failed|exception` 부재 + assistant_response 길이 > 임계
- `friction`: 같은 도구 재호출이 있었으면 시행착오로 학습 가치 ↑
- `user_signal`: consolidation이 이미 키워드 부스트("decision|always|never|critical")를 함 — 그 시그널을 재활용

### 5.4 Bit 3 — SkillTrigger 게이트

`agent/loop_.rs` 턴 종료 후:

```rust
let trace = capture_trace(turn);
let signal = evaluate_value(&trace, &skill_index).await?;

if score(&signal) >= config.skill_trigger_threshold {
    tokio::spawn(async move {
        let result = creator.create_from_execution(
            &trace.user_message,
            &trace.tool_calls,
            embedding_provider.clone(),
        ).await;
        if let Ok(new_skill) = result {
            skill_registry.hot_reload(new_skill).await;
        }
    });
}
```

**원칙**: `tokio::spawn`으로 *fire-and-forget*. 메모리 consolidation과 같은 패턴 (`ws.rs::consolidate_turn` 참고). 사용자 응답 지연 0.

### 5.5 Bit 4 — Hot Reload

**문제**: 현재 `load_skills()`는 부팅 시 1회. 같은 세션에서 만든 스킬을 다음 턴에 못 쓴다.

**해법**: `SkillRegistry` 래퍼 도입.

```rust
pub struct SkillRegistry {
    skills: Arc<RwLock<Vec<Skill>>>,
    tools: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
    prompt_cache: Arc<RwLock<Option<String>>>,  // 무효화 플래그
}

impl SkillRegistry {
    pub async fn hot_reload(&self, new_skill: Skill) {
        // 1. audit 재실행
        // 2. skills 벡터에 추가 (중복 시 교체)
        // 3. tools 맵 갱신
        // 4. prompt_cache 무효화 → 다음 skills_to_prompt() 호출 시 재생성
    }
}
```

기존 `skills_to_prompt()` / `skills_to_tools()`를 `SkillRegistry`의 메서드로 옮기면 호출자는 변경 없음.

### 5.6 Bit 5 — Memory ↔ Skill 브리지

**현재**: `consolidation::consolidate_turn()`이 매 턴 LLM으로 `history_entry` + `memory_update`를 뽑는다 (`consolidation.rs:55-136`).

**확장**: 추출 프롬프트에 *세 번째 카테고리* "skill_candidate"를 추가.

```
당신은 대화를 분석해 다음을 추출합니다:
1. history_entry: 일일 로그
2. memory_update: Core 메모리 후보
3. skill_candidate: 재사용 가능한 *절차*가 발견되면 슬러그/요약/예시 (없으면 null)
```

`skill_candidate` 출력은 ValueSignal의 `user_signal` 입력으로 흘러간다. 즉 LLM이 "이건 절차적이다"라고 판단한 케이스를 메모리 레이어가 *사전 마크*해서 Bit 3 게이트의 정확도를 끌어올린다.

**역방향**: 스킬 생성 직후, 생성된 SKILL.md의 핵심 요약을 `MemoryEntry { category: Custom("skill_index"), namespace, importance: 0.8 }`로도 저장. 의미 검색(`retrieval::recall`) 시 스킬이 함께 잡혀, 사용자의 자연어 질의에 스킬이 매칭될 확률 ↑.

### 5.7 Bit 6 — 사용 통계

**무엇을**: 스킬이 prompt에 노출/도구로 호출/성공/실패한 횟수.

**어디에**: `MemoryEntry { category: Custom("skill_stat"), key: slug, content: JSON, importance: 0.4 }` — 별도 테이블 신설하지 않고 메모리에 얹는다.

**왜 메모리에**: decay가 자연스럽게 적용 → 안 쓰는 스킬은 점수가 떨어져 자동 폐기 후보가 된다.

**개선 트리거**: 실패율 ≥ 임계 OR 사용 빈도가 상위 5%인데 마지막 개선 > 30일 → `SkillImprover::improve_skill()` 호출 (별도 batch tick).

### 5.8 Bit 7 — SkillForge 스케줄러 (보너스)

`scan_interval_hours` 설정만 있고 실행 안 됨(`skillforge/mod.rs:31`). `naraeclaw-runtime/src/cron/`이 이미 있으므로 cron 등록 1줄로 백그라운드 진화 가능.

```rust
// runtime 부팅 시
if config.skillforge.enabled {
    cron::register_job(
        format!("0 */{} * * *", config.skillforge.scan_interval_hours),
        || async { skillforge.forge().await }
    );
}
```

---

## 6. 인터페이스 변경 요약

새로 도입(신규):
- `agent/execution_trace.rs::ExecutionTrace`
- `agent/value_signal.rs::{ValueSignal, score}`
- `skills/registry.rs::SkillRegistry` (기존 `skills_to_prompt/tools`를 메서드화)
- `skills/stats.rs::SkillStats`
- `skills/format/{native.rs, hermes.rs, convert.rs, mod.rs}` — **D5 결정 반영**. 양쪽 포맷 동시 지원 변환 레이어. native(SKILL.toml/SKILL.md)는 1급 시민, hermes(SKILL.md 프론트매터)는 import/export 시 변환
- `naraeclaw-tools/src/mark_skill_candidate.rs` — **D2 결정 반영**. 에이전트가 명시 신호를 보낼 수 있는 도구 (config로 자동 노출)

기존 변경(최소):
- `agent/loop_.rs` — 턴 종료 시점에 trace 캡처 + 게이트 호출 (≤30줄)
- `memory/consolidation.rs` — 추출 프롬프트에 `skill_candidate` 필드 추가 (프롬프트 텍스트 + 파싱) — **D6 결정에 따라 비용 +10–15% 허용**
- `skills/mod.rs::load_skills` — `format::auto_detect()`로 위임, native/hermes 모두 인식 → `SkillRegistry`로 감싸기
- `skillforge/mod.rs` — cron 등록 1지점, hermes 포맷 후보도 통합

호환 fallback 없이 진행 가능 (config 기본값 off로 출시 → 안정 후 on).

**Config 키 신설** (CLAUDE.md "Config 키는 공개 계약" 규칙에 따라 기본값·마이그레이션 문서화 필수):
```toml
[agent.execution_trace]
enabled = false                  # M1 기본 off — trace 캡처 자체 토글

[skills.auto_evolution]
enabled = true                   # 기본 on — 게이트만 통과. 실제 SkillCreator
                                 # 동작은 [skills.skill_creation].enabled에 의존
                                 # (그쪽은 여전히 기본 false라 신규 동작 없음)
trigger_threshold = 0.6          # D1 — ValueSignal 임계
hot_reload = true                # M3
user_signal_keyword = true       # D2 — consolidation에서 키워드 추출
user_signal_tool = true          # D2 — mark_skill_candidate 도구 노출

[skills.auto_evolution.stats]
# D4 — decay 자연 폐기. prompt 노출 임계만 둔다 (archive 명령 없음)
prompt_exposure_min_score = 0.1

[skills.format]
# D5 — 양쪽 포맷 지원
accept_hermes_md = true          # import 시 허용
export_hermes_md = false         # 기본은 NaraeClaw 고유, 명시적으로 export

[skillforge.scheduler]
enabled = false                  # M4 기본 off
```

---

## 7. 마일스톤

각 마일스톤은 별도 `claude/<작업명>` worktree에서 진행, 작업 파일이 겹치지 않게 분할.

### M1 — 추적 인프라 (저위험)
- `ExecutionTrace` 캡처 + `agent/loop_.rs` 훅
- `SkillStats` 메모리 적재 (`Custom("skill_stat")` 카테고리)
- **검증**: 단위 테스트 + live 한 세션 돌려 trace JSON 덤프 확인
- **범위**: trace 캡처만, 자동 생성 OFF. `agent.execution_trace.enabled` flag로 보호

### M2 — 자동 트리거 (중위험)
- `ValueSignal` + `score()` + **D1 임계값 0.6** 기본
- `SkillCreator` 자동 호출 (`skills.auto_evolution.enabled` flag로 보호)
- **검증**: MockProvider로 멀티스텝 시나리오 통과, 무한 루프 가드(스킬 생성 동안 다시 트리거 차단)

### M3 — 핫 리로드 + Memory 브리지 (중위험)
- `SkillRegistry` 도입, 기존 호출자 마이그레이션
- `consolidation` 프롬프트에 `skill_candidate` 추가 — **D6 비용 +10–15% 수용**
- **D2** `mark_skill_candidate` 도구 신설 (auto_approve 목록 검토)
- **D3** 스킬 인덱스 메모리 적재 (`Custom("skill_index")`, importance 0.8)
- **검증**: 같은 세션에서 생성→재사용 시나리오, consolidation 회귀 fixture replay

### M4 — SkillForge 스케줄러 (저위험, 보너스)
- cron 1회 등록 (`skillforge.scheduler.enabled` flag)
- **검증**: 짧은 주기(테스트용)로 도는지 확인

### M5 — 포맷 호환 레이어 (저위험, D5 신규)
- `skills/format/{native, hermes, convert, mod}.rs` 신설
- `load_skills` → `format::auto_detect()` 위임 (SKILL.toml vs SKILL.md 프론트매터 자동 분기)
- Hermes → NaraeClaw 변환: 프론트매터 → SKILL.toml, body → SKILL.md
- NaraeClaw → Hermes export 커맨드 (`naraeclaw skills export --format hermes <slug>`)
- **M1~M4와 파일 충돌 없음** — 병렬 worktree 가능. M3 머지 후 시작이 자연스럽지만 순서 강제 아님
- **검증**: agentskills.io 샘플 스킬 import → load → export round-trip 테스트

---

## 8. 리스크

| 리스크 | 영향 | 완화 |
|---|---|---|
| 노이즈 스킬 폭증 | 프롬프트 비대, 의사결정 마비 | ValueSignal 임계 + LRU(creator 이미 있음) + decay 기반 자동 폐기 |
| 자동 트리거가 잘못된 절차를 굳힘 | 잘못된 학습 | `improver` 쿨다운(이미 있음) + 사용 통계 기반 개선·롤백 |
| 핫 리로드 race condition | 같은 슬러그 동시 생성 | `SkillRegistry`에 `RwLock` + 슬러그 단일 키 mutex |
| consolidation 프롬프트 변경이 기존 memory_update 추출 정확도 저하 | 메모리 품질 회귀 | M3 전에 fixture replay (`TraceLlmProvider`) 회귀 테스트 |
| `tokio::spawn` 누수 | 백그라운드 작업 폭증 | 세마포어로 동시 생성 ≤ N 제한 |

---

## 9. 참고 자료

- Hermes Agent 공식: https://hermes-agent.org/
- NousResearch 해설: https://discuss.pytorch.kr/t/hermes-agent-nousresearch-ai/9184
- 기존 ADR: `docs/architecture/adr-004-tool-shared-state-ownership.md`

---

## 변경 이력

- **2026-05-13 (제안)** — 초기 작성, Status: Proposed, 오픈 질문 6개 (Q1~Q6)
- **2026-05-13 (확정)** — Status: Accepted. D1~D6 결정 확정 (`Decision` 섹션 참조). M5 신설(포맷 호환 레이어). Section 8 오픈 질문 제거 → `Decision` 표로 흡수
