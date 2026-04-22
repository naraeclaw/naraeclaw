# 구현 계획: Knowledge Ingestion Pipeline

## 개요

NaraeClaw의 지식 수집 파이프라인을 구현한다. 다양한 소스(로컬 파일, Slack, Notion, Google Drive, 이메일, Confluence)에서 콘텐츠를 수집하고, LLM으로 분석·요약·구조화하여 WikiMemory에 마크다운으로 저장하는 전체 흐름을 `naraeclaw-config`(설정 스키마)와 `naraeclaw-runtime`(파이프라인 로직)에 걸쳐 구현한다.

## Tasks

- [ ] 1. 설정 스키마 및 핵심 데이터 모델 정의
  - [ ] 1.1 `naraeclaw-config`에 `IngestionConfig` 설정 스키마 추가
    - `crates/naraeclaw-config/src/schema/config_types.rs`에 `IngestionConfig`, `SourceConfig`, `IngestionContextConfig`, `GitSnapshotConfig` 구조체 추가
    - `Configurable` derive 매크로 적용, `serde(default)` 기본값 설정
    - 기존 `Config` 구조체에 `pub ingestion: IngestionConfig` 필드 추가
    - `crates/naraeclaw-config/src/schema/mod.rs`에서 새 타입 re-export
    - _요구사항: 13.1, 13.2, 13.3, 13.4, 13.5, 13.6, 13.7, 13.8_

  - [ ] 1.2 `naraeclaw-runtime`에 ingestion 모듈 구조 및 핵심 타입 생성
    - `crates/naraeclaw-runtime/src/ingestion/mod.rs` 생성 및 `lib.rs`에 `pub mod ingestion;` 추가
    - `crates/naraeclaw-runtime/src/ingestion/types.rs`에 `SourceType`, `RawContent`, `ContentMetadata`, `FetchParams`, `IngestionJob`, `IngestionResult`, `IngestionError`, `TriggerType` 정의
    - `crates/naraeclaw-runtime/src/ingestion/connector.rs`에 `SourceConnector` 트레이트 정의 (`fetch`, `health_check`, `name`)
    - _요구사항: 1, 2, 3, 4, 5, 6, 7_

  - [ ] 1.3 카테고리 매핑 함수 구현
    - `crates/naraeclaw-runtime/src/ingestion/category.rs`에 `resolve_category(source_type, target_id, override_category)` 함수 구현
    - 소스 유형별 기본 매핑: Local → `Custom("local-{folder}")`, Slack → `Custom("slack-{channel}")`, Notion → `Custom("notion-{page}")`, GDrive → `Custom("gdrive-{folder}")`, Email → `Custom("email-{folder}")`, Confluence → `Custom("confluence-{space}")`, GenericHttp → `Custom("http-{host}")`
    - `wiki_category` 오버라이드 지원
    - _요구사항: 8.2, 8.3_

  - [ ]* 1.4 카테고리 매핑 속성 테스트 작성
    - **속성 6: 카테고리 매핑 일관성** — 동일한 `source_type`과 `target_id`에 대해 `resolve_category`는 항상 동일한 값을 반환해야 한다
    - **검증 대상: 요구사항 8.2**

  - [ ]* 1.5 설정 스키마 단위 테스트 작성
    - `IngestionConfig` 기본값 역직렬화 테스트
    - `SourceConfig` 필수/선택 필드 검증 테스트
    - 인증 정보 누락 시 경고 로직 테스트
    - _요구사항: 13.1, 13.7_

- [ ] 2. 체크포인트 — 설정 스키마 및 핵심 타입 검증
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

- [ ] 3. `ContentChunker` 및 `DedupHashStore` 구현
  - [ ] 3.1 `ContentChunker` 구현
    - `crates/naraeclaw-runtime/src/ingestion/chunker.rs`에 `ContentChunker` 구조체 및 `chunk()` 메서드 구현
    - 문단/줄바꿈 경계에서 분할하여 의미 단위 보존
    - `max_tokens` (기본 4,000), `context_window` (기본 128,000) 설정 지원
    - 각 `ContentChunk`에 `index`, `total`, `text`, `source_id` 포함
    - _요구사항: 1.3, 7.2_

  - [ ] 3.2 `DedupHashStore` 구현
    - `crates/naraeclaw-runtime/src/ingestion/dedup.rs`에 `DedupHashStore` 구조체 구현
    - `wiki/.ingestion-state.json` 파일에 `IngestionState` (last_runs, content_hashes) 영속화
    - `is_changed(content_id, new_hash)`, `update_hash(content_id, hash)`, `last_run(source_id)`, `update_last_run(source_id, time)` 메서드 구현
    - SHA-256 해시 계산 유틸리티 함수 포함
    - _요구사항: 1.6, 1.7, 8.6_

  - [ ]* 3.3 `DedupHashStore` 속성 테스트 작성
    - **속성 7: 수집 상태 파일 단조 증가** — `update_last_run` 호출 후 `last_run` 값은 이전 값보다 항상 크거나 같아야 한다
    - **검증 대상: 요구사항 8.6**

  - [ ]* 3.4 `DedupHashStore` 중복 방지 속성 테스트 작성
    - **속성 2: Dedup_Hash 중복 방지** — 동일한 해시를 가진 콘텐츠에 대해 `is_changed`는 false를 반환해야 한다
    - **검증 대상: 요구사항 1.6, 5.6**

  - [ ]* 3.5 `ContentChunker` 단위 테스트 작성
    - 작은 콘텐츠(분할 불필요) 테스트
    - 대용량 콘텐츠 분할 시 청크 수 및 경계 검증
    - 빈 콘텐츠 처리 테스트
    - _요구사항: 1.3, 7.2_

- [ ] 4. `LlmAnalyzer` 구현
  - [ ] 4.1 `LlmAnalyzer` 핵심 로직 구현
    - `crates/naraeclaw-runtime/src/ingestion/analyzer.rs`에 `LlmAnalyzer` 구조체 구현
    - `analyze(chunk, source_meta)` — `Provider` 트레이트를 통해 LLM 호출, `AnalyzedSection` 벡터 반환
    - `merge_sections(existing, new_content)` — 기존 위키 섹션과 새 내용 병합 프롬프트 실행
    - 분석 결과를 `## {주제}` H2 섹션 형식으로 구조화
    - 소스 유형과 수집 날짜를 메타데이터로 각 섹션에 포함
    - _요구사항: 7.1, 7.3, 7.4, 7.6_

  - [ ] 4.2 LLM 분석 프롬프트 템플릿 시스템 구현
    - 소스 유형별 기본 프롬프트 템플릿 정의 (Slack, Notion, 로컬 파일, 이메일, Confluence, Google Drive)
    - `[ingestion.prompts]` 설정에서 소스 유형별 프롬프트 재정의 지원
    - `[ingestion]` 섹션의 `model` 설정으로 분석용 LLM 모델 선택 (기본값: 에이전트 기본 모델)
    - LLM API 호출 실패 시 오류 기록 후 나머지 청크 처리 계속
    - _요구사항: 7.5, 7.7, 7.8_

  - [ ]* 4.3 LLM 분석 결과 구조 속성 테스트 작성
    - **속성 3: LLM 분석 결과 구조 불변식** — `LlmAnalyzer`가 반환하는 결과는 항상 하나 이상의 H2 섹션을 포함해야 한다
    - **검증 대상: 요구사항 7.3**

- [ ] 5. 체크포인트 — 핵심 파이프라인 컴포넌트 검증
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

- [ ] 6. 소스 커넥터 구현 — 로컬 파일 및 Slack
  - [ ] 6.1 `LocalFileConnector` 구현
    - `crates/naraeclaw-runtime/src/ingestion/connectors/mod.rs` 생성 및 커넥터 모듈 구조 설정
    - `crates/naraeclaw-runtime/src/ingestion/connectors/local.rs`에 `LocalFileConnector` 구현
    - `SecurityPolicy`를 통한 경로 접근 제어
    - 지원 확장자 필터링 (`.md`, `.txt`, `.rst`, `.org`, `.pdf`)
    - `exclude_patterns` glob 매칭으로 파일 제외
    - 10MB 초과 파일 감지 및 `ContentChunker` 연계
    - SHA-256 `Dedup_Hash` 계산 및 `mtime` 비교로 증분 처리
    - 존재하지 않는 경로/읽기 권한 없는 경로 오류 처리
    - _요구사항: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 14.2_

  - [ ] 6.2 `SlackConnector` 구현
    - `crates/naraeclaw-runtime/src/ingestion/connectors/slack.rs`에 `SlackConnector` 구현
    - 기존 `SlackChannel`의 봇 토큰을 `naraeclaw-config`에서 재사용
    - `conversations.history` API + cursor 기반 페이지네이션
    - `since` 파라미터로 증분 수집
    - `include_threads` 옵션으로 스레드 답글 포함 여부 제어
    - HTTP 429 시 `Retry-After` 헤더 기반 대기 후 재시도
    - 사용자 ID → 표시 이름 변환 (`users.info` API 캐싱)
    - 봇 토큰 미설정 시 오류 반환
    - _요구사항: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7_

  - [ ]* 6.3 `LocalFileConnector` 단위 테스트 작성
    - 단일 파일 읽기 테스트
    - 디렉토리 재귀 열거 테스트
    - `exclude_patterns` 필터링 테스트
    - 존재하지 않는 경로 오류 처리 테스트
    - `Dedup_Hash` 중복 건너뛰기 테스트
    - _요구사항: 1.1, 1.2, 1.4, 1.5, 1.6_

  - [ ]* 6.4 소스 커넥터 멱등성 속성 테스트 작성 (LocalFile)
    - **속성 1: 소스 커넥터 멱등성** — 동일한 `IngestionJob`을 두 번 실행한 결과 위키 파일 상태는 한 번 실행한 것과 동일해야 한다 (LocalFileConnector 대상)
    - **검증 대상: 요구사항 1.6**

- [ ] 7. 소스 커넥터 구현 — Notion, Google Drive, 이메일
  - [ ] 7.1 `NotionConnector` 구현
    - `crates/naraeclaw-runtime/src/ingestion/connectors/notion.rs`에 `NotionConnector` 구현
    - 기존 `NotionChannel`의 API 키 재사용
    - 페이지: `/v1/blocks/{id}/children` 재귀 수집
    - 데이터베이스: `/v1/databases/{id}/query` → 각 페이지 수집
    - Notion 블록 타입 → 마크다운 변환
    - `last_edited_time` 비교로 증분 처리
    - 실패 시 최대 3회 지수 백오프 (2초 기본 지연)
    - _요구사항: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

  - [ ] 7.2 `GDriveConnector` 구현
    - `crates/naraeclaw-runtime/src/ingestion/connectors/gdrive.rs`에 `GDriveConnector` 구현
    - OAuth 2.0 토큰을 `secrets` 섹션에서 로드
    - `files.export` (Google Docs/Sheets/Slides → plain text/markdown)
    - `files.list` (폴더 내 파일 열거)
    - 토큰 만료 시 리프레시 토큰으로 자동 갱신
    - `modifiedTime` 비교로 증분 처리
    - 자격증명 미설정 시 오류 반환
    - _요구사항: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7_

  - [ ] 7.3 `EmailConnector` 구현
    - `crates/naraeclaw-runtime/src/ingestion/connectors/email.rs`에 `EmailConnector` 구현
    - 기존 `EmailChannel`의 IMAP 설정 재사용
    - Gmail API 설정 존재 시 `GmailPushChannel`의 OAuth 자격증명 재사용
    - HTML → plain text 변환
    - 텍스트 첨부 파일 추출 (`.txt`, `.md`, `.pdf`)
    - `since` 파라미터로 날짜 기반 증분 수집
    - Message-ID 기반 중복 방지
    - IMAP 연결 실패 시 오류 기록
    - _요구사항: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7_

  - [ ]* 7.4 Notion/GDrive/Email 커넥터 단위 테스트 작성
    - 각 커넥터의 인증 정보 누락 시 오류 처리 테스트
    - 증분 처리 로직 (last_edited_time, modifiedTime, Message-ID) 테스트
    - _요구사항: 3.5, 3.6, 4.5, 4.6, 4.7, 5.6, 5.7_

- [ ] 8. 소스 커넥터 구현 — Confluence 및 Generic HTTP
  - [ ] 8.1 `ConfluenceConnector` 구현
    - `crates/naraeclaw-runtime/src/ingestion/connectors/confluence.rs`에 `ConfluenceConnector` 구현
    - `/rest/api/content` REST API로 페이지 수집
    - XHTML 스토리지 형식 → 마크다운 변환
    - 기본 인증 또는 API 토큰 (`secrets` 섹션)
    - `version.number` / `lastModified` 비교로 증분 처리
    - 자격증명 미설정 시 오류 반환
    - _요구사항: 6.1, 6.2, 6.3, 6.5, 6.6_

  - [ ] 8.2 `GenericHttpConnector` 구현
    - `crates/naraeclaw-runtime/src/ingestion/connectors/generic_http.rs`에 `GenericHttpConnector` 구현
    - 사용자 지정 HTTP 엔드포인트에서 마크다운/HTML 수집
    - `wiki_type = "generic_http"` 설정 시 활성화
    - _요구사항: 6.4_

  - [ ]* 8.3 Confluence/GenericHttp 커넥터 단위 테스트 작성
    - Confluence XHTML → 마크다운 변환 테스트
    - 자격증명 누락 시 오류 처리 테스트
    - GenericHttp 기본 수집 테스트
    - _요구사항: 6.2, 6.6_

- [ ] 9. 체크포인트 — 전체 소스 커넥터 검증
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

- [ ] 10. `IngestionCoordinator` 구현
  - [ ] 10.1 `IngestionCoordinator` 핵심 로직 구현
    - `crates/naraeclaw-runtime/src/ingestion/coordinator.rs`에 `IngestionCoordinator` 구조체 구현
    - `run_job(job)` — 소스 검증 → 커넥터 fetch → dedup 확인 → chunking → LLM 분석 → WikiMemory 저장 전체 흐름
    - `create_connector(source_config, config)` — 소스 유형에 따라 적절한 커넥터 인스턴스 생성
    - `running_jobs` 뮤텍스로 동일 소스 동시 실행 방지
    - 수집 완료 후 `Observer`를 통해 이벤트 기록 (`IngestionStarted`, `IngestionCompleted`, `IngestionFailed`)
    - 처리된 항목 수, 생성/업데이트된 위키 섹션 수, 소요 시간 로그 기록
    - _요구사항: 8.1, 8.4, 8.5, 8.6, 10.4, 10.5, 14.1, 14.5_

  - [ ] 10.2 WikiMemory 저장 연계 구현
    - `WikiMemory.store(key, content, category)` 인터페이스를 통한 분석 결과 저장
    - `resolve_category` 함수로 소스 유형 기반 카테고리 결정
    - 사용자 `wiki_category` 설정 오버라이드 지원
    - 저장 실패 시 해당 섹션 건너뛰고 오류 기록 후 나머지 계속
    - 수집 완료 후 `wiki/.ingestion-state.json`에 마지막 성공 실행 시각 기록
    - _요구사항: 8.1, 8.2, 8.3, 8.5, 8.6_

  - [ ] 10.3 보안 경계 및 수집 범위 제한 구현
    - `Ingestion_Config`에 등록된 소스만 수집 허용, 미등록 소스 요청 거부
    - 로컬 파일 수집 시 `naraeclaw-runtime/security/` 정책에 따른 경로 접근 제어
    - 원본 콘텐츠를 `wiki/` 외부에 영구 저장하지 않음 (임시 파일 처리 후 즉시 삭제)
    - 인증 토큰을 로그나 위키 파일에 기록하지 않음
    - `mask_pii` 설정에 따른 PII 마스킹 지원
    - _요구사항: 14.1, 14.2, 14.3, 14.4, 14.5, 14.6_

  - [ ]* 10.4 보안 경계 속성 테스트 작성
    - **속성 8: 보안 경계 불변식** — `Ingestion_Config`에 등록되지 않은 소스에 대한 수집은 항상 거부되어야 한다
    - **검증 대상: 요구사항 14.1, 14.5**

  - [ ]* 10.5 수집 → 저장 → recall 라운드트립 속성 테스트 작성
    - **속성 4: 수집 → 저장 → recall 라운드트립** — 수집 후 저장된 내용은 `WikiMemory.recall`로 검색 가능해야 한다
    - **검증 대상: 요구사항 8.1, 12.1**

  - [ ]* 10.6 소스 커넥터 멱등성 통합 속성 테스트 작성
    - **속성 1: 소스 커넥터 멱등성** — 동일한 `IngestionJob`을 두 번 실행한 결과 위키 파일 상태는 한 번 실행한 것과 동일해야 한다 (IngestionCoordinator 전체 흐름 대상)
    - **검증 대상: 요구사항 1.6, 2.6, 3.6, 4.6, 5.6**

- [ ] 11. 위키 재정리 (Wiki Refactor) 구현
  - [ ] 11.1 위키 재정리 기능 구현
    - `IngestionCoordinator`에 `plan_refactor()` 메서드 구현 — 현재 `wiki/` 디렉토리 파일을 읽고 LLM에게 최적 구조 제안 요청
    - `execute_refactor(plan)` 메서드 구현 — 승인된 계획에 따라 `WikiMemory.store`, `WikiMemory.forget`으로 파일 이동/섹션 재배치
    - 재정리 실행 전 Git 커밋으로 현재 상태 스냅샷 저장
    - 재정리 중 오류 발생 시 작업 중단 및 오류 발생 전 상태 복원
    - 재정리 완료 후 변경된 파일 목록과 작업 요약 보고
    - _요구사항: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6_

  - [ ]* 11.2 위키 재정리 단위 테스트 작성
    - 재정리 계획 생성 테스트
    - 재정리 실행 전 스냅샷 생성 확인 테스트
    - 오류 시 롤백 테스트
    - _요구사항: 9.4, 9.5_

- [ ] 12. 체크포인트 — 코디네이터 및 재정리 검증
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

- [ ] 13. 주기적 자동 수집 및 일일 Git 스냅샷 구현
  - [ ] 13.1 주기적 자동 수집 (Ingestion Schedule) 구현
    - `crates/naraeclaw-runtime/src/ingestion/schedule.rs`에 스케줄 등록 로직 구현
    - `naraeclaw-runtime/src/cron/` 기존 cron 스케줄러에 수집 작업 등록
    - `schedule` 설정이 있는 소스에 대해 cron 표현식(5필드) 파싱 및 등록
    - cron 트리거 시 `IngestionJob` 생성, 마지막 성공 수집 이후 변경분만 처리
    - 동일 소스 실행 중일 때 새 작업 건너뛰기
    - `notify_on_complete = true` 시 완료 알림 전송
    - `schedule` 미설정 소스는 수동 요청 시에만 수집
    - _요구사항: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7_

  - [ ] 13.2 `DailySnapshot` 구현
    - `crates/naraeclaw-runtime/src/ingestion/snapshot.rs`에 `DailySnapshot` 구조체 구현
    - `std::process::Command`로 로컬 `git` 바이너리 호출
    - `git diff --quiet wiki/` → 변경사항 확인
    - 변경사항 있을 때: `git add wiki/` → `git commit -m "[나래] YYYY-MM-DD 일일 지식 스냅샷"`
    - 변경사항 없을 때: 커밋 없이 종료
    - `git` 바이너리 미존재 또는 Git 저장소 아닌 경우 오류 기록 후 종료
    - Git 커밋 결과를 `Observer`를 통해 기록 (`DailySnapshotResult`)
    - `[ingestion.git_snapshot.enabled]`가 `false`이면 등록하지 않음
    - `[ingestion.git_snapshot.schedule]` 설정으로 기본 자정 스케줄 재정의 가능
    - cron 스케줄러에 등록 (기본: `0 0 * * *`)
    - _요구사항: 11.1, 11.2, 11.3, 11.4, 11.5, 11.6, 11.7, 11.8, 11.9_

  - [ ]* 13.3 일일 스냅샷 속성 테스트 작성
    - **속성 5: 일일 스냅샷 커밋 조건** — 변경사항이 있을 때만 Git 커밋이 생성되어야 한다
    - **검증 대상: 요구사항 11.3, 11.4**

  - [ ]* 13.4 스케줄 등록 및 DailySnapshot 단위 테스트 작성
    - cron 표현식 파싱 테스트
    - 동일 소스 동시 실행 방지 테스트
    - `enabled = false` 시 스냅샷 미등록 테스트
    - _요구사항: 10.1, 10.4, 11.8_

- [ ] 14. 에이전트 컨텍스트 주입 구현
  - [ ] 14.1 `WikiContextInjector` 구현
    - `crates/naraeclaw-runtime/src/ingestion/context.rs`에 `WikiContextInjector` 구조체 구현
    - `build_context(query)` — `WikiMemory.recall(query, limit)` 호출로 관련 위키 섹션 검색, 컨텍스트 문자열 생성
    - `max_recall_sections` 설정으로 반환 섹션 수 제한 (기본 5)
    - recall 결과 없을 때 빈 컨텍스트 반환
    - `[ingestion.context.enabled]`가 `false`이면 컨텍스트 주입 비활성화
    - _요구사항: 12.1, 12.2, 12.3, 12.4, 12.6_

  - [ ] 14.2 에이전트 루프에 위키 컨텍스트 주입 연결
    - `crates/naraeclaw-channels/src/orchestrator/mod.rs`의 기존 메모리 recall 흐름과 독립적으로 동작하도록 위키 컨텍스트 주입 포인트 추가
    - 시스템 프롬프트 또는 컨텍스트 메시지로 주입 방식 설정 가능
    - _요구사항: 12.2, 12.5_

  - [ ]* 14.3 `WikiContextInjector` 단위 테스트 작성
    - recall 결과 있을 때 컨텍스트 문자열 생성 테스트
    - recall 결과 없을 때 빈 컨텍스트 반환 테스트
    - `enabled = false` 시 비활성화 테스트
    - `max_recall_sections` 제한 테스트
    - _요구사항: 12.1, 12.3, 12.4, 12.6_

- [ ] 15. ObserverEvent 확장 및 웹 대시보드 UI
  - [ ] 15.1 `ObserverEvent` enum에 수집 관련 이벤트 추가
    - `crates/naraeclaw-runtime/src/observability/traits.rs`에 `IngestionStarted`, `IngestionCompleted`, `IngestionFailed`, `DailySnapshotResult` 이벤트 추가
    - 기존 Observer 구현체들(log, verbose, otel 등)에 새 이벤트 핸들링 추가
    - _요구사항: 10.5, 11.7_

  - [ ] 15.2 수집 파이프라인 웹 대시보드 페이지 구현
    - `web/src/pages/Ingestion.tsx`에 수집 파이프라인 상태 모니터링 페이지 구현
    - 파이프라인 상태 요약 카드 (총 수집 수, 오늘 수집 수, 오류 수)
    - 소스 목록 카드 그리드 (아이콘, 이름, 상태 배지, 마지막 수집 시각, 다음 스케줄, 프로그레스 바)
    - 최근 수집 작업 목록 (작업 ID, 소스, 트리거 유형, 시작/완료 시각, 상태 배지, 처리 항목 수)
    - Git 스냅샷 상태 (마지막 커밋 해시, 다음 스냅샷 예정 시각)
    - 다크 모드 우선, 기존 `--pc-*` CSS 변수 시스템 활용
    - 소스별 아이콘 및 색상 적용
    - SSE 실시간 업데이트 (`useSSE` 훅)
    - _요구사항: 10.5, 11.7_

  - [ ]* 15.3 웹 대시보드 단위 테스트 작성
    - 컴포넌트 렌더링 테스트
    - 소스 카드 상태 배지 표시 테스트
    - _요구사항: 10.5_

- [ ] 16. 체크포인트 — 전체 통합 검증
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

- [ ] 17. 전체 연결 및 최종 통합
  - [ ] 17.1 `naraeclaw-runtime` ingestion 모듈 공개 API 정리
    - `crates/naraeclaw-runtime/src/ingestion/mod.rs`에서 모든 하위 모듈 re-export 정리
    - `IngestionCoordinator`, `WikiContextInjector`, `DailySnapshot`을 에이전트 초기화 흐름에 연결
    - `naraeclaw-runtime/src/lib.rs`에서 ingestion 모듈 공개
    - _요구사항: 전체_

  - [ ] 17.2 에이전트 시작 시 수집 파이프라인 초기화 연결
    - 에이전트 시작 시 `IngestionConfig` 로드 및 검증
    - 필수 인증 정보 누락 소스에 대해 경고 출력 및 비활성화
    - `schedule` 설정된 소스를 cron 스케줄러에 등록
    - `DailySnapshot`을 cron 스케줄러에 등록 (enabled 시)
    - `WikiContextInjector`를 에이전트 루프에 연결
    - _요구사항: 13.7, 10.2, 11.1, 12.5_

  - [ ]* 17.3 전체 통합 테스트 작성
    - MockProvider를 사용한 수집 → 분석 → 저장 → recall 전체 흐름 테스트
    - 설정 로드 → 커넥터 생성 → 수집 실행 통합 테스트
    - _요구사항: 전체_

- [ ] 18. 최종 체크포인트 — 전체 테스트 통과 확인
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

## 참고 사항

- `*` 표시된 태스크는 선택 사항이며 빠른 MVP를 위해 건너뛸 수 있습니다
- 각 태스크는 추적 가능성을 위해 특정 요구사항을 참조합니다
- 체크포인트는 증분 검증을 보장합니다
- 속성 테스트는 설계 문서의 정확성 속성을 검증합니다
- 단위 테스트는 특정 예제와 엣지 케이스를 검증합니다
- 이 프로젝트는 Rust (edition 2024)로 구현되며, `naraeclaw-config`와 `naraeclaw-runtime` 크레이트에 걸쳐 작업합니다
