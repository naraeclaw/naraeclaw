# 구현 계획: Wiki Memory Backend

## 개요

`WikiMemory` 백엔드를 `naraeclaw-memory` 크레이트에 추가한다. `MemoryBackendKind::Wiki` variant 등록, `wiki.rs` 모듈 구현, 팩토리 통합, 설정 확장, CLI 마이그레이션 도구 순서로 점진적으로 구현한다. 각 단계는 이전 단계 위에 빌드되며, 마지막에 전체를 연결한다.

## Tasks

- [ ] 1. 백엔드 등록 및 설정 확장
  - [ ] 1.1 `MemoryBackendKind`에 `Wiki` variant 추가 및 분류 함수 업데이트
    - `crates/naraeclaw-memory/src/backend.rs`에 `Wiki` variant 추가
    - `classify_memory_backend("wiki")`가 `MemoryBackendKind::Wiki`를 반환하도록 매치 분기 추가
    - `WIKI_PROFILE` 상수 정의 (`key: "wiki"`, `label: "Wiki Files — topic-based markdown, human-editable, no embeddings"`, `auto_save_default: true`, `uses_sqlite_hygiene: false`, `sqlite_based: false`, `optional_dependency: false`)
    - `memory_backend_profile("wiki")`가 `WIKI_PROFILE`을 반환하도록 매치 분기 추가
    - `SELECTABLE_MEMORY_BACKENDS` 배열에 `WIKI_PROFILE` 추가 (markdown과 none 사이)
    - _Requirements: 5.3, 5.4_

  - [ ] 1.2 `MemoryConfig`에 `wiki_dir` 필드 추가
    - `crates/naraeclaw-config/src/schema/config_types.rs`의 `MemoryConfig`에 `wiki_dir: Option<String>` 필드 추가
    - `#[serde(default, skip_serializing_if = "Option::is_none")]` 어트리뷰트 적용
    - 주석: `/// Custom wiki directory path (relative to workspace root). Only used when backend = "wiki". Default: "wiki".`
    - _Requirements: 5.5_

  - [ ]* 1.3 백엔드 등록 단위 테스트 작성
    - `classify_memory_backend("wiki")` == `MemoryBackendKind::Wiki` 확인
    - `selectable_memory_backends()`에 wiki 프로파일 포함 확인
    - `memory_backend_profile("wiki")` 프로파일 필드 검증
    - 기존 백엔드(`sqlite`, `lucid`, `markdown`, `qdrant`, `none`) 분류가 변경되지 않았는지 회귀 테스트
    - _Requirements: 5.3, 5.4, 6.1–6.5_

- [ ] 2. `WikiMemory` 핵심 구현
  - [ ] 2.1 `WikiMemory` 구조체 및 내부 유틸리티 함수 구현
    - `crates/naraeclaw-memory/src/wiki.rs` 파일 생성
    - `WikiMemory` 구조체 정의 (`wiki_dir: PathBuf`)
    - `WikiSection` 내부 구조체 정의 (`key: String`, `content: String`)
    - `category_to_filename(category: &MemoryCategory) -> String` 구현 (Core→core.md, Daily→daily.md, Conversation→conversation.md, Custom(name)→{sanitized_name}.md)
    - `sanitize_key(key: &str) -> String` 구현 (`/\:*?"<>|` → `_` 치환)
    - `parse_wiki_file(content: &str) -> Vec<WikiSection>` 구현 (H2 섹션 파싱)
    - `render_wiki_file(title: &str, sections: &[WikiSection]) -> String` 구현 (H1 + H2 섹션 렌더링)
    - `WikiMemory::new(wiki_dir: &Path) -> Self` 생성자 구현
    - _Requirements: 1.1, 1.6, 1.7, 3.5_

  - [ ]* 2.2 키 새니타이징 property 테스트 작성
    - **Property 6: 키 새니타이징**
    - `proptest`로 안전하지 않은 문자 포함 임의 문자열에 대해 `sanitize_key` 결과에 `/\:*?"<>|` 문자가 없음을 검증
    - **Validates: Requirements 3.5**

  - [ ]* 2.3 파싱/렌더링 라운드트립 property 테스트 작성
    - **Property 2: 위키 파일 구조 불변식**
    - `proptest`로 임의의 WikiSection 시퀀스에 대해 `render_wiki_file` → `parse_wiki_file` 라운드트립 후 H1 헤더 1개, 동일 key 섹션 최대 1개, 섹션 내용 보존 검증
    - **Validates: Requirements 1.6, 1.7, 1.8, 3.4**

- [ ] 3. `Memory` 트레이트 구현 — store, recall, get
  - [ ] 3.1 `store` 메서드 구현
    - `wiki_dir` 디렉토리 자동 생성 (`tokio::fs::create_dir_all`)
    - 카테고리에 대응하는 위키 파일 읽기 (없으면 H1 헤더로 새 파일 생성)
    - `parse_wiki_file`로 기존 섹션 파싱
    - 동일 key 섹션이 있으면 content 교체, 없으면 새 섹션 추가
    - `render_wiki_file`로 렌더링 후 디스크에 즉시 기록 (`tokio::fs::write`)
    - _Requirements: 1.1–1.8, 3.1–3.5_

  - [ ] 3.2 `recall` 메서드 구현
    - 모든 위키 파일 스캔 (빈 쿼리: 전체 반환, 키워드 쿼리: 파일명/내용 매칭)
    - 각 WikiSection을 `MemoryEntry`로 변환 (id, key, content, category, timestamp, namespace)
    - 키워드 매칭 점수 기반 정렬 후 `limit`으로 truncate
    - 파일이 없거나 삭제된 경우 빈 결과 반환 (오류 아님)
    - _Requirements: 2.1–2.7, 8.1, 8.4_

  - [ ] 3.3 `get`, `list`, `forget`, `count`, `health_check`, `name` 메서드 구현
    - `name()`: `"wiki"` 반환
    - `get(key)`: 모든 위키 파일에서 key 매칭 섹션 검색
    - `list(category, session_id)`: 카테고리별 필터링된 전체 섹션 반환
    - `forget(key)`: 모든 위키 파일에서 해당 key 섹션 삭제, 삭제 성공 시 `true`, 미존재 시 `false` 반환. 삭제 후 파일 구조(H1 헤더) 유지
    - `count()`: 전체 섹션 수 반환
    - `health_check()`: `wiki_dir` 존재 여부 반환
    - _Requirements: 2.6, 2.7, 4.1–4.5, 5.2_

  - [ ]* 3.4 Store → Recall 라운드트립 property 테스트 작성
    - **Property 3: Store → Recall 라운드트립**
    - `proptest`로 임의의 key, content, category에 대해 `store` 후 `recall(key, 100, ...)` 결과에 해당 항목 포함 검증
    - **Validates: Requirements 2.1, 2.2, 2.6, 2.7, 3.1, 3.3**

  - [ ]* 3.5 카테고리 → 파일 매핑 일관성 property 테스트 작성
    - **Property 1: 카테고리 → 파일 매핑 일관성**
    - `proptest`로 임의의 MemoryCategory, key, content에 대해 `store` 후 해당 카테고리 파일에 섹션 존재 검증
    - **Validates: Requirements 1.2, 1.3, 1.4, 1.5**

  - [ ]* 3.6 Recall Limit 상한 property 테스트 작성
    - **Property 4: Recall Limit 상한**
    - `proptest`로 임의의 limit (0..100)에 대해 `recall` 결과 길이 ≤ limit 검증
    - **Validates: Requirements 2.3**

  - [ ]* 3.7 Store 멱등성 property 테스트 작성
    - **Property 5: Store 멱등성 (단일 섹션 보장)**
    - `proptest`로 동일 (key, category)에 대해 N번 (1..10) `store` 후 해당 파일에서 H2 섹션 정확히 1개, 마지막 content와 동일 검증
    - **Validates: Requirements 3.2**

  - [ ]* 3.8 Store → Forget → Get 라운드트립 property 테스트 작성
    - **Property 7: Store → Forget → Get 라운드트립**
    - `proptest`로 임의의 key, content에 대해 `store` → `forget` → `get` == None 검증, forget 후 파일 구조 유지 검증
    - **Validates: Requirements 4.1, 4.2, 4.4, 4.5**

  - [ ]* 3.9 외부 편집 즉시 반영 property 테스트 작성
    - **Property 9: 외부 편집 즉시 반영 (캐시 없음)**
    - `proptest`로 `store` 후 파일을 직접 수정하여 content 변경 → `get` 결과가 변경된 content 반영 검증
    - **Validates: Requirements 8.1, 8.2**

  - [ ]* 3.10 외부 콘텐츠 보존 property 테스트 작성
    - **Property 10: 외부 콘텐츠 보존**
    - `proptest`로 위키 파일에 추가 마크다운 콘텐츠 삽입 후 `store`/`forget` 연산 시 해당 콘텐츠 보존 검증
    - **Validates: Requirements 8.3**

  - [ ]* 3.11 단위 테스트 작성 (example-based)
    - 빈 쿼리로 `recall` 시 모든 섹션 반환 (2.4)
    - 매칭 없는 쿼리로 `recall` 시 빈 벡터 반환 (2.5)
    - 존재하지 않는 키로 `forget` 시 `false` 반환 (4.3)
    - `wiki_dir` 존재하지 않을 때 `health_check` → `false` (5.6)
    - 파일 삭제 후 `recall` 시 빈 결과 (8.4)
    - _Requirements: 2.4, 2.5, 4.3, 5.6, 8.4_

- [ ] 4. 체크포인트 — 핵심 WikiMemory 검증
  - 모든 테스트가 통과하는지 확인하고, 질문이 있으면 사용자에게 문의한다.

- [ ] 5. 팩토리 통합 및 모듈 등록
  - [ ] 5.1 `wiki` 모듈 등록 및 팩토리 함수 통합
    - `crates/naraeclaw-memory/src/lib.rs`에 `pub mod wiki;` 추가 및 `pub use wiki::WikiMemory;` 추가
    - `create_memory_with_builders()`에 `MemoryBackendKind::Wiki` 분기 추가
    - `MemoryConfig.wiki_dir`이 설정된 경우 해당 경로 사용, 미설정 시 `workspace_dir.join("wiki")` 기본값 사용
    - _Requirements: 5.1, 5.5, 5.6_

  - [ ]* 5.2 팩토리 통합 테스트 작성
    - `create_memory()` 팩토리에서 `backend = "wiki"` 시 `WikiMemory` 인스턴스 생성 확인 (`name() == "wiki"`)
    - `wiki_dir` 설정 오버라이드 동작 확인
    - 기존 백엔드(`sqlite`, `markdown`, `lucid`, `none`) 팩토리 회귀 테스트 (기존 테스트 활용)
    - _Requirements: 5.1, 5.5, 6.1–6.7_

- [ ] 6. CLI `export-wiki` 마이그레이션 도구 구현
  - [ ] 6.1 `MemoryCommands`에 `ExportWiki` variant 추가 및 핸들러 구현
    - `src/lib.rs`의 `MemoryCommands`에 `ExportWiki { dry_run: bool }` variant 추가
    - `src/memory/cli.rs`에 `handle_export_wiki(config, dry_run)` 핸들러 구현
    - SQLite 메모리에서 전체 항목 읽기 → 카테고리별 위키 파일로 내보내기
    - 기존 위키 파일이 있으면 병합 (동일 키 섹션은 건너뛰기)
    - 개별 항목 변환 실패 시 경고 출력 후 건너뛰기
    - 완료 후 처리된 항목 수와 생성된 파일 목록 출력
    - `--dry-run` 플래그: 실제 파일 생성 없이 항목 수와 파일 목록만 출력
    - _Requirements: 7.1–7.6_

  - [ ]* 6.2 마이그레이션 내용 보존 property 테스트 작성
    - **Property 8: 마이그레이션 내용 보존**
    - `proptest`로 임의의 MemoryEntry 집합에 대해 마이그레이션 후 각 항목의 key/content가 위키 파일에 존재하고, 기존 위키 섹션이 보존됨을 검증
    - **Validates: Requirements 7.2, 7.4**

  - [ ]* 6.3 CLI 마이그레이션 단위 테스트 작성
    - `export-wiki` CLI 명령 존재 확인 (7.1)
    - `--dry-run` 플래그 동작 확인 (7.6)
    - SQLite → Wiki end-to-end 마이그레이션 테스트
    - _Requirements: 7.1, 7.6_

- [ ] 7. 최종 체크포인트 — 전체 통합 검증
  - 모든 테스트가 통과하는지 확인하고, 질문이 있으면 사용자에게 문의한다.
  - `cargo clippy --all-targets -- -D warnings` 통과 확인
  - `cargo fmt --all -- --check` 통과 확인

## Notes

- `*` 표시된 태스크는 선택 사항이며, 빠른 MVP를 위해 건너뛸 수 있습니다.
- 각 태스크는 추적 가능성을 위해 특정 요구사항을 참조합니다.
- 체크포인트는 점진적 검증을 보장합니다.
- Property 테스트는 `proptest` 크레이트를 사용하여 정확성 속성을 검증합니다.
- 단위 테스트는 특정 예제와 엣지 케이스를 검증합니다.
- 기존 백엔드에 대한 회귀 테스트는 기존 테스트 코드를 활용합니다.
