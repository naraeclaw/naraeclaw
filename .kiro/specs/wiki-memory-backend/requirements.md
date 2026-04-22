# 요구사항 문서

## 소개

Andrej Karpathy가 제안한 위키 기반 지식 보관소 메모리 백엔드(`WikiMemory`)를 NaraeClaw에 추가한다.

기존 메모리 백엔드(SQLite, Markdown, Qdrant)는 벡터 임베딩이나 단일 파일 append-only 방식에 의존한다. `WikiMemory`는 이와 달리 `wiki/` 디렉토리 아래 주제별 마크다운 파일로 지식을 구조화하여, 에이전트가 파일 전체를 LLM 컨텍스트로 직접 읽고 섹션 단위로 편집할 수 있게 한다. 벡터 임베딩이 필요 없고, 사람이 직접 편집할 수 있어 투명성과 감사 가능성이 높다.

이 백엔드는 기존 백엔드를 대체하지 않고 `backend = "wiki"` 설정으로 선택 가능한 추가 옵션으로 제공된다.

---

## 용어 정의

- **WikiMemory**: 이 기능에서 구현하는 위키 기반 메모리 백엔드 구조체.
- **위키 파일(Wiki_File)**: `wiki/` 디렉토리 아래 주제별로 생성되는 마크다운 파일 (예: `wiki/user.md`, `wiki/projects.md`).
- **위키 섹션(Wiki_Section)**: 위키 파일 내 `## 키` 형식의 H2 헤더로 구분되는 단위. 하나의 메모리 항목에 대응한다.
- **카테고리(MemoryCategory)**: 메모리 항목의 분류. `Core`, `Daily`, `Conversation`, `Custom(String)` 중 하나.
- **파일 매핑(File_Mapping)**: `MemoryCategory`와 위키 파일명 사이의 대응 관계 (예: `Core` → `core.md`, `Custom("projects")` → `projects.md`).
- **Memory_Factory**: `create_memory()` 팩토리 함수. `MemoryConfig.backend` 값에 따라 적절한 백엔드 인스턴스를 생성한다.
- **Migration_Tool**: 기존 SQLite 메모리를 위키 형식으로 내보내는 CLI 도구.
- **MemoryConfig**: `naraeclaw-config` 크레이트의 메모리 설정 구조체. `backend` 필드로 백엔드를 선택한다.

---

## 요구사항

### 요구사항 1: 위키 파일 구조

**사용자 스토리:** 개발자로서, 에이전트의 메모리가 주제별 마크다운 파일로 구조화되기를 원한다. 그래야 사람이 직접 읽고 편집할 수 있기 때문이다.

#### 인수 기준

1. THE `WikiMemory` SHALL `wiki/` 디렉토리를 워크스페이스 루트 아래에 생성하고 모든 위키 파일을 해당 디렉토리에 저장한다.
2. WHEN `MemoryCategory::Core` 항목이 저장될 때, THE `WikiMemory` SHALL 해당 항목을 `wiki/core.md` 파일에 기록한다.
3. WHEN `MemoryCategory::Daily` 항목이 저장될 때, THE `WikiMemory` SHALL 해당 항목을 `wiki/daily.md` 파일에 기록한다.
4. WHEN `MemoryCategory::Conversation` 항목이 저장될 때, THE `WikiMemory` SHALL 해당 항목을 `wiki/conversation.md` 파일에 기록한다.
5. WHEN `MemoryCategory::Custom(name)` 항목이 저장될 때, THE `WikiMemory` SHALL 해당 항목을 `wiki/{name}.md` 파일에 기록한다. 단, `name`은 파일 시스템에 안전한 문자만 포함해야 한다.
6. THE `WikiMemory` SHALL 각 위키 파일의 첫 줄에 `# {주제명}` 형식의 H1 헤더를 유지한다.
7. THE `WikiMemory` SHALL 각 메모리 항목을 `## {key}` 형식의 H2 헤더와 그 아래 본문 텍스트로 구성된 위키 섹션으로 저장한다.
8. IF `wiki/` 디렉토리가 존재하지 않을 때, THEN THE `WikiMemory` SHALL 첫 번째 `store` 호출 시 해당 디렉토리를 자동으로 생성한다.

**정확성 속성 (Property-Based Testing):**

- **파일 매핑 일관성**: 임의의 `MemoryCategory`와 키에 대해 `store` 후 해당 카테고리에 대응하는 위키 파일에 항목이 존재해야 한다. `∀ category, key, content: store(key, content, category) → file_for(category).contains_section(key)`
- **파일 구조 불변식**: 임의의 `store` 연산 후 모든 위키 파일은 유효한 마크다운 구조(H1 헤더 1개, 0개 이상의 H2 섹션)를 유지해야 한다.

---

### 요구사항 2: 에이전트 읽기 (recall)

**사용자 스토리:** 에이전트로서, 쿼리와 관련된 위키 파일 전체를 컨텍스트로 받기를 원한다. 그래야 LLM이 임베딩 없이 직접 내용을 이해할 수 있기 때문이다.

#### 인수 기준

1. WHEN `recall(query, limit, ...)` 이 호출될 때, THE `WikiMemory` SHALL 쿼리 키워드가 파일명 또는 파일 내용에 포함된 위키 파일들을 검색한다.
2. WHEN 관련 위키 파일이 발견될 때, THE `WikiMemory` SHALL 해당 파일의 각 위키 섹션을 개별 `MemoryEntry`로 변환하여 반환한다.
3. THE `WikiMemory` SHALL 반환되는 `MemoryEntry` 목록을 `limit` 개수로 제한한다.
4. WHEN 쿼리가 빈 문자열일 때, THE `WikiMemory` SHALL 모든 위키 파일의 모든 섹션을 `MemoryEntry`로 반환한다 (limit 적용).
5. WHEN 어떤 위키 파일도 쿼리와 일치하지 않을 때, THE `WikiMemory` SHALL 빈 벡터를 반환한다.
6. THE `WikiMemory` SHALL 각 `MemoryEntry`의 `key` 필드를 해당 위키 섹션의 H2 헤더 텍스트로 설정한다.
7. THE `WikiMemory` SHALL 각 `MemoryEntry`의 `content` 필드를 해당 위키 섹션의 본문 텍스트로 설정한다.

**정확성 속성 (Property-Based Testing):**

- **내용 보존**: `store(key, content, category)` 후 `recall(key, ...)` 결과에 해당 항목이 포함되어야 한다. `∀ key, content: store(key, content) → recall(key).any(|e| e.key == key && e.content == content)`
- **limit 상한 보장**: 임의의 `limit` 값에 대해 `recall` 결과의 길이는 항상 `limit` 이하여야 한다. `∀ limit ≥ 0: recall(query, limit).len() ≤ limit`

---

### 요구사항 3: 에이전트 쓰기 (store)

**사용자 스토리:** 에이전트로서, 새로운 지식을 적절한 위키 파일의 섹션으로 저장하거나 기존 섹션을 업데이트하기를 원한다. 그래야 지식이 중복 없이 최신 상태로 유지되기 때문이다.

#### 인수 기준

1. WHEN `store(key, content, category, session_id)` 가 호출될 때, THE `WikiMemory` SHALL 해당 카테고리에 대응하는 위키 파일에 `## {key}` 섹션을 추가하거나 기존 섹션을 업데이트한다.
2. WHEN 동일한 `key`로 `store`가 두 번 호출될 때, THE `WikiMemory` SHALL 기존 섹션의 내용을 새 내용으로 교체하고 중복 섹션을 생성하지 않는다.
3. THE `WikiMemory` SHALL `store` 완료 후 위키 파일을 디스크에 즉시 기록한다.
4. IF 위키 파일이 존재하지 않을 때, THEN THE `WikiMemory` SHALL 파일을 새로 생성하고 H1 헤더를 추가한 후 섹션을 기록한다.
5. THE `WikiMemory` SHALL `key` 값에서 파일 시스템에 안전하지 않은 문자(`/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`)를 `_`로 치환하여 섹션 헤더에 사용한다.

**정확성 속성 (Property-Based Testing):**

- **멱등성(Idempotence)**: 동일한 `(key, content, category)`로 `store`를 여러 번 호출해도 결과 파일 상태는 한 번 호출한 것과 동일해야 한다. `∀ key, content, category: store(k,c,cat); store(k,c,cat) ≡ store(k,c,cat)`
- **단일 섹션 보장**: 임의의 키로 `store`를 N번 호출한 후 해당 파일에서 그 키에 해당하는 H2 섹션은 정확히 1개여야 한다. `∀ key, N ≥ 1: count_sections(key) == 1`

---

### 요구사항 4: 섹션 삭제 (forget)

**사용자 스토리:** 에이전트 또는 사용자로서, 특정 메모리 항목을 위키에서 삭제하기를 원한다. 그래야 오래되거나 잘못된 정보를 제거할 수 있기 때문이다.

#### 인수 기준

1. WHEN `forget(key)` 가 호출될 때, THE `WikiMemory` SHALL 모든 위키 파일에서 `## {key}` 섹션과 그 본문을 삭제한다.
2. WHEN `forget(key)` 가 성공적으로 섹션을 삭제했을 때, THE `WikiMemory` SHALL `true`를 반환한다.
3. WHEN `forget(key)` 가 호출되었으나 해당 키의 섹션이 존재하지 않을 때, THE `WikiMemory` SHALL `false`를 반환한다.
4. WHEN 섹션 삭제 후 위키 파일에 H1 헤더만 남고 섹션이 없을 때, THE `WikiMemory` SHALL 해당 파일을 유지한다 (빈 파일 삭제 금지).
5. THE `WikiMemory` SHALL `forget` 완료 후 변경된 위키 파일을 디스크에 즉시 기록한다.

**정확성 속성 (Property-Based Testing):**

- **store → forget 라운드트립**: `store(key, content)` 후 `forget(key)` 를 호출하면 `get(key)` 는 `None`을 반환해야 한다. `∀ key, content: store(key, content); forget(key) → get(key) == None`
- **forget 후 파일 구조 유지**: `forget` 후에도 위키 파일은 유효한 마크다운 구조를 유지해야 한다.

---

### 요구사항 5: 설정 통합

**사용자 스토리:** 운영자로서, `config.toml`에서 `backend = "wiki"` 한 줄로 위키 메모리 백엔드를 활성화하기를 원한다. 그래야 기존 설정 방식과 일관성을 유지할 수 있기 때문이다.

#### 인수 기준

1. WHEN `MemoryConfig.backend` 가 `"wiki"` 로 설정될 때, THE `Memory_Factory` SHALL `WikiMemory` 인스턴스를 생성하여 반환한다.
2. THE `WikiMemory` SHALL `Memory` 트레이트의 `name()` 메서드에서 `"wiki"` 를 반환한다.
3. THE `MemoryBackendKind` SHALL `Wiki` 변형(variant)을 포함하고, `classify_memory_backend("wiki")` 는 `MemoryBackendKind::Wiki`를 반환해야 한다.
4. THE `selectable_memory_backends()` SHALL `WikiMemory` 프로파일을 포함하여 온보딩 UI에서 선택 가능하게 한다.
5. WHERE `[memory]` 섹션에 `wiki_dir` 키가 설정된 경우, THE `WikiMemory` SHALL 해당 경로를 `wiki/` 디렉토리 기본 경로 대신 사용한다.
6. WHEN `backend = "wiki"` 로 설정되었으나 `wiki/` 디렉토리 생성에 실패할 때, THE `Memory_Factory` SHALL 오류를 반환하고 폴백 없이 실패한다.

**정확성 속성 (Property-Based Testing):**

- **팩토리 분기 일관성**: `classify_memory_backend("wiki")` 가 `Wiki`를 반환하면 `create_memory` 는 항상 `name() == "wiki"` 인 인스턴스를 반환해야 한다.

---

### 요구사항 6: 기존 백엔드와의 공존

**사용자 스토리:** 기존 사용자로서, `wiki` 백엔드 추가 후에도 기존 `sqlite`, `markdown`, `qdrant`, `lucid`, `none` 백엔드가 동일하게 동작하기를 원한다. 그래야 기존 설정을 변경하지 않아도 되기 때문이다.

#### 인수 기준

1. THE `Memory_Factory` SHALL `backend = "sqlite"` 설정 시 기존과 동일하게 `SqliteMemory` 인스턴스를 반환한다.
2. THE `Memory_Factory` SHALL `backend = "markdown"` 설정 시 기존과 동일하게 `MarkdownMemory` 인스턴스를 반환한다.
3. THE `Memory_Factory` SHALL `backend = "qdrant"` 설정 시 기존과 동일하게 `QdrantMemory` 인스턴스를 반환한다.
4. THE `Memory_Factory` SHALL `backend = "lucid"` 설정 시 기존과 동일하게 `LucidMemory` 인스턴스를 반환한다.
5. THE `Memory_Factory` SHALL `backend = "none"` 설정 시 기존과 동일하게 `NoneMemory` 인스턴스를 반환한다.
6. THE `WikiMemory` SHALL 기존 `MarkdownMemory`의 `MEMORY.md` 및 `memory/` 디렉토리를 읽거나 수정하지 않는다.
7. THE `WikiMemory` SHALL `naraeclaw-memory` 크레이트에 새로운 필수 외부 의존성을 추가하지 않는다 (기존 `tokio::fs`, `chrono` 등 활용).

---

### 요구사항 7: SQLite → 위키 마이그레이션 도구

**사용자 스토리:** 기존 SQLite 메모리 사용자로서, 축적된 메모리를 위키 형식으로 내보내기를 원한다. 그래야 위키 백엔드로 전환할 때 기존 지식을 잃지 않기 때문이다.

#### 인수 기준

1. THE `Migration_Tool` SHALL `naraeclaw memory export-wiki` CLI 서브커맨드로 실행 가능해야 한다.
2. WHEN `export-wiki` 가 실행될 때, THE `Migration_Tool` SHALL 현재 설정된 SQLite 메모리의 모든 항목을 읽어 카테고리별 위키 파일로 내보낸다.
3. THE `Migration_Tool` SHALL 내보내기 완료 후 처리된 항목 수와 생성된 파일 목록을 표준 출력에 출력한다.
4. IF 대상 위키 파일이 이미 존재할 때, THEN THE `Migration_Tool` SHALL 기존 파일을 덮어쓰지 않고 새 항목을 기존 파일에 병합(merge)한다. 단, 동일 키의 섹션이 이미 존재하면 건너뛴다.
5. WHEN `export-wiki` 실행 중 개별 항목 변환에 실패할 때, THE `Migration_Tool` SHALL 해당 항목을 건너뛰고 경고를 출력한 후 나머지 항목 처리를 계속한다.
6. THE `Migration_Tool` SHALL `--dry-run` 플래그를 지원하며, 이 경우 실제 파일을 생성하지 않고 내보낼 항목 수와 파일 목록만 출력한다.

**정확성 속성 (Property-Based Testing):**

- **내용 보존**: 임의의 SQLite 메모리 항목 집합에 대해 마이그레이션 후 각 항목의 `key`와 `content`가 위키 파일에 그대로 존재해야 한다. `∀ entries: migrate(entries) → ∀ e ∈ entries: wiki_file_for(e.category).contains_section(e.key, e.content)`
- **항목 수 보존**: 마이그레이션 후 위키 파일 전체의 섹션 수는 원본 항목 수 이상이어야 한다 (기존 위키 항목이 있을 경우 더 많을 수 있음). `∀ entries: section_count_after ≥ entries.len()`

---

### 요구사항 8: 사람이 직접 편집 가능성

**사용자 스토리:** 사용자로서, 텍스트 편집기로 위키 파일을 직접 수정한 후 에이전트가 변경된 내용을 즉시 반영하기를 원한다. 그래야 에이전트 메모리를 투명하게 감사하고 수정할 수 있기 때문이다.

#### 인수 기준

1. THE `WikiMemory` SHALL 메모리 항목을 인메모리 캐시 없이 매 `recall`, `get`, `list` 호출 시 디스크에서 직접 읽는다.
2. WHEN 위키 파일이 외부 편집기에 의해 수정된 후 `recall` 이 호출될 때, THE `WikiMemory` SHALL 수정된 내용을 반영한 결과를 반환한다.
3. THE `WikiMemory` SHALL 위키 파일에 에이전트가 생성하지 않은 추가 마크다운 콘텐츠(H2 섹션 외의 텍스트, 코드 블록 등)가 있을 때 해당 내용을 보존한다.
4. IF 위키 파일이 외부에서 삭제된 후 `recall` 이 호출될 때, THEN THE `WikiMemory` SHALL 오류를 반환하지 않고 빈 결과를 반환한다.
