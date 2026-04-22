# 요구사항 문서

## 소개

NaraeClaw의 지식 수집 파이프라인(`knowledge-ingestion-pipeline`)은 사용자가 지정한 다양한 소스(로컬 파일, Slack, Notion, Google Drive, 이메일, Confluence 등 위키)의 콘텐츠를 LLM이 분석·요약·구조화하여 `wiki/` 디렉토리(WikiMemory 백엔드)에 마크다운 파일로 저장하는 기능이다.

에이전트는 대화 중 이 위키를 자동으로 참조하여 개인화된 응답을 제공하며, 매일 자정에 `wiki/` 변경사항을 Git으로 자동 커밋하여 에이전트의 지식 성장 이력을 추적한다.

**핵심 원칙**:
- 사용자가 명시적으로 지시한 것만 처리 (자동 수집 없음, 단 폴더별 주기 설정은 가능)
- LLM이 원본을 읽고 요약/구조화 → WikiMemory에 저장
- 위키 폴더 재정리(리팩터링)도 에이전트가 수행 가능
- 에이전트가 지식을 축적할수록 사용자와 함께 개인화

이 기능은 `wiki-memory-backend` 스펙에서 정의한 `WikiMemory` 백엔드를 저장소로 사용하며, 기존 `naraeclaw-channels`의 Slack, Notion, 이메일 채널 구현을 소스 커넥터로 재활용한다.

---

## 용어 정의

- **Ingestion_Pipeline**: 소스에서 콘텐츠를 수집하고 LLM으로 분석하여 WikiMemory에 저장하는 전체 처리 흐름.
- **Source_Connector**: 특정 소스(로컬 파일, Slack, Notion, Google Drive, 이메일, 위키)에서 원본 콘텐츠를 가져오는 컴포넌트.
- **Ingestion_Job**: 단일 수집 작업 단위. 소스 유형, 대상 식별자, 수집 범위, 실행 시각을 포함한다.
- **LLM_Analyzer**: 원본 콘텐츠를 받아 주제별 요약·구조화된 마크다운을 생성하는 컴포넌트.
- **WikiMemory**: `wiki/` 디렉토리 기반 마크다운 메모리 백엔드 (`wiki-memory-backend` 스펙 참조).
- **Wiki_Refactor**: 에이전트가 기존 위키 파일들을 읽고 재구조화(파일 이동·병합·분리)하는 작업.
- **Ingestion_Schedule**: 소스별로 설정된 cron 표현식 기반 자동 수집 주기.
- **Daily_Snapshot**: 매일 자정에 `wiki/` 변경사항을 Git으로 커밋하는 작업.
- **Ingestion_Config**: `naraeclaw-config`의 TOML 설정에서 수집 파이프라인을 정의하는 섹션.
- **Content_Chunk**: LLM 컨텍스트 윈도우 제한을 고려하여 원본 콘텐츠를 분할한 단위.
- **Dedup_Hash**: 콘텐츠 중복 수집을 방지하기 위해 원본 콘텐츠에서 계산한 해시값.

---

## 요구사항

### 요구사항 1: 로컬 파일/폴더 소스 커넥터

**사용자 스토리:** 사용자로서, 로컬 파일이나 폴더를 지정하면 에이전트가 그 내용을 분석하여 위키에 저장하기를 원한다. 그래야 오프라인 문서도 에이전트의 지식 베이스에 포함할 수 있기 때문이다.

#### 인수 기준

1. WHEN 사용자가 로컬 파일 경로를 수집 대상으로 지정할 때, THE `Source_Connector` SHALL 해당 파일의 내용을 UTF-8 텍스트로 읽어 `Ingestion_Pipeline`에 전달한다.
2. WHEN 사용자가 로컬 디렉토리 경로를 수집 대상으로 지정할 때, THE `Source_Connector` SHALL 해당 디렉토리 아래의 모든 텍스트 파일(`.md`, `.txt`, `.rst`, `.org`, `.pdf` 텍스트 레이어)을 재귀적으로 열거하여 각각 `Ingestion_Pipeline`에 전달한다.
3. THE `Source_Connector` SHALL 단일 파일 크기가 10MB를 초과할 때 해당 파일을 `Content_Chunk` 단위(기본 4,000 토큰 추정)로 분할하여 처리한다.
4. IF 지정된 경로가 존재하지 않거나 읽기 권한이 없을 때, THEN THE `Source_Connector` SHALL 오류 메시지를 반환하고 해당 경로를 건너뛴다.
5. WHERE `exclude_patterns` 설정이 존재할 때, THE `Source_Connector` SHALL glob 패턴과 일치하는 파일을 수집 대상에서 제외한다.
6. THE `Source_Connector` SHALL 각 파일의 `Dedup_Hash`(SHA-256)를 계산하여 이전 수집 시 동일한 해시가 기록된 파일은 재처리하지 않는다.
7. WHEN 파일이 마지막 수집 이후 수정된 경우, THE `Source_Connector` SHALL 해당 파일을 재수집 대상으로 포함한다.

---

### 요구사항 2: Slack 소스 커넥터

**사용자 스토리:** 사용자로서, Slack 채널의 메시지 이력을 분석하여 위키에 저장하기를 원한다. 그래야 팀의 논의와 결정 사항이 에이전트의 지식 베이스에 축적되기 때문이다.

#### 인수 기준

1. WHEN 사용자가 Slack 채널 ID 또는 채널명을 수집 대상으로 지정할 때, THE `Source_Connector` SHALL 기존 `SlackChannel`의 봇 토큰을 재사용하여 `conversations.history` API로 메시지를 수집한다.
2. THE `Source_Connector` SHALL `since` 파라미터가 지정된 경우 해당 타임스탬프 이후의 메시지만 수집한다.
3. THE `Source_Connector` SHALL Slack API 응답의 페이지네이션(`cursor`)을 처리하여 지정된 기간의 모든 메시지를 수집한다.
4. THE `Source_Connector` SHALL 스레드 답글을 포함하여 수집할지 여부를 `include_threads` 설정으로 제어한다.
5. IF Slack API 호출이 속도 제한(HTTP 429)에 도달할 때, THEN THE `Source_Connector` SHALL `Retry-After` 헤더에 명시된 시간만큼 대기 후 재시도한다.
6. THE `Source_Connector` SHALL 수집된 메시지에서 사용자 ID를 표시 이름으로 변환하여 LLM 분석 시 가독성을 높인다.
7. IF Slack 봇 토큰이 설정되지 않았을 때, THEN THE `Source_Connector` SHALL 오류를 반환하고 수집을 중단한다.

---

### 요구사항 3: Notion 소스 커넥터

**사용자 스토리:** 사용자로서, Notion 페이지나 데이터베이스의 내용을 분석하여 위키에 저장하기를 원한다. 그래야 Notion에 정리된 지식이 에이전트와 연결되기 때문이다.

#### 인수 기준

1. WHEN 사용자가 Notion 페이지 ID를 수집 대상으로 지정할 때, THE `Source_Connector` SHALL Notion API(`/v1/blocks/{id}/children`)로 해당 페이지의 블록 트리를 재귀적으로 수집한다.
2. WHEN 사용자가 Notion 데이터베이스 ID를 수집 대상으로 지정할 때, THE `Source_Connector` SHALL `/v1/databases/{id}/query`로 모든 페이지를 열거하고 각 페이지의 내용을 수집한다.
3. THE `Source_Connector` SHALL Notion 블록 타입(paragraph, heading, bulleted_list_item, code, table 등)을 마크다운으로 변환하여 `LLM_Analyzer`에 전달한다.
4. THE `Source_Connector` SHALL 기존 `NotionChannel`의 API 키 설정을 재사용한다.
5. IF Notion API 호출이 실패할 때, THEN THE `Source_Connector` SHALL 최대 3회 지수 백오프(2초 기본 지연)로 재시도한다.
6. THE `Source_Connector` SHALL 페이지의 `last_edited_time`을 확인하여 마지막 수집 이후 변경되지 않은 페이지는 재처리하지 않는다.

---

### 요구사항 4: Google Drive 소스 커넥터

**사용자 스토리:** 사용자로서, Google Drive의 문서나 폴더 내용을 분석하여 위키에 저장하기를 원한다. 그래야 Google Workspace에서 작성한 문서도 에이전트의 지식 베이스에 포함되기 때문이다.

#### 인수 기준

1. WHEN 사용자가 Google Drive 파일 ID를 수집 대상으로 지정할 때, THE `Source_Connector` SHALL Google Drive API(`files.export` 또는 `files.get`)로 해당 파일의 텍스트 내용을 수집한다.
2. WHEN 사용자가 Google Drive 폴더 ID를 수집 대상으로 지정할 때, THE `Source_Connector` SHALL `files.list` API로 폴더 내 모든 파일을 열거하고 각각의 내용을 수집한다.
3. THE `Source_Connector` SHALL Google Docs, Sheets, Slides를 각각 plain text 또는 마크다운으로 변환하여 수집한다.
4. THE `Source_Connector` SHALL OAuth 2.0 액세스 토큰을 `naraeclaw-config`의 `secrets` 섹션에서 읽어 인증에 사용한다.
5. IF Google Drive API 인증 토큰이 만료되었을 때, THEN THE `Source_Connector` SHALL 리프레시 토큰으로 액세스 토큰을 갱신한 후 재시도한다.
6. THE `Source_Connector` SHALL 파일의 `modifiedTime`을 확인하여 마지막 수집 이후 변경되지 않은 파일은 재처리하지 않는다.
7. IF Google Drive API 자격증명이 설정되지 않았을 때, THEN THE `Source_Connector` SHALL 오류를 반환하고 수집을 중단한다.

---

### 요구사항 5: 이메일 소스 커넥터

**사용자 스토리:** 사용자로서, 이메일 받은편지함의 내용을 분석하여 위키에 저장하기를 원한다. 그래야 이메일로 받은 중요한 정보가 에이전트의 지식 베이스에 포함되기 때문이다.

#### 인수 기준

1. WHEN 사용자가 이메일 폴더(IMAP 메일박스)를 수집 대상으로 지정할 때, THE `Source_Connector` SHALL 기존 `EmailChannel`의 IMAP 설정을 재사용하여 해당 폴더의 이메일을 수집한다.
2. THE `Source_Connector` SHALL `since` 파라미터가 지정된 경우 해당 날짜 이후 수신된 이메일만 수집한다.
3. THE `Source_Connector` SHALL 이메일 본문에서 HTML 태그를 제거하고 plain text로 변환하여 `LLM_Analyzer`에 전달한다.
4. THE `Source_Connector` SHALL 이메일 첨부 파일 중 텍스트 형식(`.txt`, `.md`, `.pdf` 텍스트 레이어)을 추출하여 본문과 함께 처리한다.
5. WHERE Gmail API 설정이 존재할 때, THE `Source_Connector` SHALL 기존 `GmailPushChannel`의 OAuth 자격증명을 재사용하여 Gmail API로 이메일을 수집한다.
6. THE `Source_Connector` SHALL 수집된 이메일의 Message-ID를 `Dedup_Hash`로 사용하여 동일 이메일의 중복 처리를 방지한다.
7. IF IMAP 연결에 실패할 때, THEN THE `Source_Connector` SHALL 오류를 기록하고 해당 수집 작업을 실패로 표시한다.

---

### 요구사항 6: 외부 위키(Confluence 등) 소스 커넥터

**사용자 스토리:** 사용자로서, Confluence 등 외부 위키의 페이지 내용을 분석하여 WikiMemory에 저장하기를 원한다. 그래야 팀 위키의 지식이 에이전트와 연결되기 때문이다.

#### 인수 기준

1. WHEN 사용자가 Confluence 스페이스 키 또는 페이지 ID를 수집 대상으로 지정할 때, THE `Source_Connector` SHALL Confluence REST API(`/rest/api/content`)로 해당 페이지의 내용을 수집한다.
2. THE `Source_Connector` SHALL Confluence 페이지의 스토리지 형식(XHTML)을 마크다운으로 변환하여 `LLM_Analyzer`에 전달한다.
3. THE `Source_Connector` SHALL Confluence API 인증 정보(기본 인증 또는 API 토큰)를 `naraeclaw-config`의 `secrets` 섹션에서 읽는다.
4. WHERE `wiki_type = "generic_http"` 설정이 존재할 때, THE `Source_Connector` SHALL 사용자가 지정한 HTTP 엔드포인트에서 마크다운 또는 HTML 콘텐츠를 수집한다.
5. THE `Source_Connector` SHALL 페이지의 `version.number` 또는 `lastModified` 필드를 확인하여 변경되지 않은 페이지는 재처리하지 않는다.
6. IF 외부 위키 API 자격증명이 설정되지 않았을 때, THEN THE `Source_Connector` SHALL 오류를 반환하고 수집을 중단한다.

---

### 요구사항 7: LLM 분석 파이프라인

**사용자 스토리:** 사용자로서, 수집된 원본 콘텐츠가 LLM에 의해 주제별로 요약·구조화되어 위키에 저장되기를 원한다. 그래야 방대한 원본 데이터가 에이전트가 활용하기 좋은 형태로 정제되기 때문이다.

#### 인수 기준

1. WHEN `Source_Connector`가 원본 콘텐츠를 전달할 때, THE `LLM_Analyzer` SHALL 해당 콘텐츠를 분석하여 주제별 요약과 핵심 정보를 추출한 마크다운 섹션을 생성한다.
2. THE `LLM_Analyzer` SHALL 단일 콘텐츠가 LLM 컨텍스트 윈도우 한계(기본 128,000 토큰)를 초과할 때 `Content_Chunk` 단위로 분할하여 순차적으로 처리하고 결과를 병합한다.
3. THE `LLM_Analyzer` SHALL 분석 결과를 `## {주제}` 형식의 H2 섹션으로 구조화하여 `WikiMemory`의 `store` 인터페이스에 맞는 형태로 반환한다.
4. THE `LLM_Analyzer` SHALL 소스 유형(Slack, 이메일, 로컬 파일 등)과 수집 날짜를 메타데이터로 각 섹션에 포함한다.
5. THE `LLM_Analyzer` SHALL 분석에 사용할 LLM 모델을 `naraeclaw-config`의 `[ingestion]` 섹션에서 설정 가능하게 한다. 기본값은 에이전트 루프에서 사용하는 기본 모델이다.
6. WHEN 동일한 주제의 섹션이 이미 위키에 존재할 때, THE `LLM_Analyzer` SHALL 기존 내용과 새 내용을 병합하는 프롬프트를 사용하여 중복 없이 업데이트한다.
7. IF LLM API 호출이 실패할 때, THEN THE `LLM_Analyzer` SHALL 오류를 기록하고 해당 `Content_Chunk`의 처리를 실패로 표시하며 나머지 청크 처리를 계속한다.
8. THE `LLM_Analyzer` SHALL 분석 프롬프트 템플릿을 `naraeclaw-config`의 `[ingestion.prompts]` 섹션에서 소스 유형별로 재정의 가능하게 한다.

---

### 요구사항 8: WikiMemory 저장 연계

**사용자 스토리:** 에이전트로서, LLM이 분석한 결과를 WikiMemory의 적절한 파일과 섹션에 저장하기를 원한다. 그래야 지식이 체계적으로 축적되고 나중에 쉽게 검색할 수 있기 때문이다.

#### 인수 기준

1. WHEN `LLM_Analyzer`가 분석 결과를 반환할 때, THE `Ingestion_Pipeline` SHALL `WikiMemory.store(key, content, category)` 인터페이스를 통해 결과를 저장한다.
2. THE `Ingestion_Pipeline` SHALL 소스 유형과 대상 식별자를 기반으로 위키 카테고리를 결정한다. 기본 매핑: Slack → `Custom("slack-{channel}")`, Notion → `Custom("notion-{page_title}")`, 로컬 파일 → `Custom("local-{folder_name}")`, 이메일 → `Custom("email-{folder}")`, Google Drive → `Custom("gdrive-{folder_name}")`.
3. THE `Ingestion_Pipeline` SHALL 사용자가 `[ingestion.sources.{source_id}]` 설정에서 `wiki_category` 키로 저장 카테고리를 명시적으로 지정할 수 있게 한다.
4. THE `Ingestion_Pipeline` SHALL 수집 완료 후 처리된 항목 수, 생성/업데이트된 위키 섹션 수, 소요 시간을 로그에 기록한다.
5. WHEN 저장 중 `WikiMemory.store`가 실패할 때, THE `Ingestion_Pipeline` SHALL 해당 섹션을 건너뛰고 오류를 기록한 후 나머지 섹션 저장을 계속한다.
6. THE `Ingestion_Pipeline` SHALL 각 수집 작업의 마지막 성공 실행 시각을 `wiki/.ingestion-state.json` 파일에 기록하여 다음 수집 시 증분 처리에 활용한다.

---

### 요구사항 9: 위키 재정리 (Wiki Refactor)

**사용자 스토리:** 사용자로서, 에이전트에게 위키 폴더 구조를 재정리해달라고 요청하기를 원한다. 그래야 시간이 지나면서 늘어난 위키 파일들이 체계적으로 관리되기 때문이다.

#### 인수 기준

1. WHEN 사용자가 위키 재정리를 요청할 때, THE `Ingestion_Pipeline` SHALL 현재 `wiki/` 디렉토리의 모든 파일을 읽고 LLM에게 최적의 구조를 제안하도록 요청한다.
2. THE `Ingestion_Pipeline` SHALL LLM이 제안한 재정리 계획(파일 이동, 섹션 병합, 파일 분리)을 사용자에게 먼저 보여주고 승인을 받은 후 실행한다.
3. WHEN 사용자가 재정리 계획을 승인할 때, THE `Ingestion_Pipeline` SHALL `WikiMemory`의 `store`, `forget` 인터페이스를 사용하여 파일 이동 및 섹션 재배치를 수행한다.
4. THE `Ingestion_Pipeline` SHALL 재정리 실행 전 `wiki/` 디렉토리의 현재 상태를 Git 커밋으로 스냅샷을 저장하여 롤백 가능하게 한다.
5. IF 재정리 중 오류가 발생할 때, THEN THE `Ingestion_Pipeline` SHALL 작업을 중단하고 오류 발생 전 상태로 복원한다.
6. THE `Ingestion_Pipeline` SHALL 재정리 완료 후 변경된 파일 목록과 수행된 작업 요약을 사용자에게 보고한다.

---

### 요구사항 10: 주기적 자동 수집 (Ingestion Schedule)

**사용자 스토리:** 사용자로서, 특정 소스를 정해진 주기로 자동 수집하도록 설정하기를 원한다. 그래야 수동으로 매번 요청하지 않아도 위키가 최신 상태로 유지되기 때문이다.

#### 인수 기준

1. THE `Ingestion_Pipeline` SHALL `naraeclaw-config`의 `[ingestion.sources.{source_id}]` 섹션에서 `schedule` 키로 cron 표현식(5필드 형식)을 설정할 수 있게 한다.
2. WHEN `schedule`이 설정된 소스가 있을 때, THE `Ingestion_Pipeline` SHALL `naraeclaw-runtime`의 기존 cron 스케줄러(`crates/naraeclaw-runtime/src/cron/`)에 해당 작업을 등록한다.
3. WHEN cron 스케줄이 도래할 때, THE `Ingestion_Pipeline` SHALL 해당 소스에 대한 `Ingestion_Job`을 생성하고 마지막 성공 수집 이후 변경된 콘텐츠만 처리한다.
4. THE `Ingestion_Pipeline` SHALL 동일 소스에 대한 수집 작업이 이미 실행 중일 때 새 작업을 시작하지 않고 건너뛴다.
5. THE `Ingestion_Pipeline` SHALL 각 자동 수집 작업의 시작, 완료, 실패를 에이전트 관찰 가능성 시스템(`naraeclaw-api/src/observability_traits.rs`)을 통해 기록한다.
6. WHERE `notify_on_complete = true` 설정이 존재할 때, THE `Ingestion_Pipeline` SHALL 자동 수집 완료 후 사용자에게 요약 알림을 전송한다.
7. THE `Ingestion_Pipeline` SHALL `schedule`이 설정되지 않은 소스는 사용자의 명시적 요청 시에만 수집을 실행한다.

---

### 요구사항 11: 일일 Git 스냅샷 (Daily Snapshot)

**사용자 스토리:** 사용자로서, 에이전트의 지식 성장 이력이 매일 Git으로 자동 커밋되기를 원한다. 그래야 언제든지 과거 특정 시점의 위키 상태로 돌아갈 수 있기 때문이다.

#### 인수 기준

1. THE `Daily_Snapshot` SHALL 매일 자정(00:00 로컬 시각)에 `naraeclaw-runtime`의 cron 스케줄러에 의해 자동 실행된다.
2. WHEN `Daily_Snapshot`이 실행될 때, THE `Daily_Snapshot` SHALL `std::process::Command`로 로컬 `git` 바이너리를 호출하여 `git diff --quiet wiki/` 명령으로 변경사항 유무를 확인한다.
3. WHEN `wiki/` 디렉토리에 변경사항이 있을 때, THE `Daily_Snapshot` SHALL `git add wiki/` 후 `git commit -m "[나래] YYYY-MM-DD 일일 지식 스냅샷"` 형식으로 커밋한다. 날짜는 로컬 시각 기준 `YYYY-MM-DD` 형식이다.
4. WHEN `wiki/` 디렉토리에 변경사항이 없을 때, THE `Daily_Snapshot` SHALL 커밋을 생성하지 않고 작업을 종료한다.
5. IF `git` 바이너리가 시스템에 존재하지 않을 때, THEN THE `Daily_Snapshot` SHALL 오류를 기록하고 커밋 없이 종료한다.
6. IF 현재 디렉토리가 Git 저장소가 아닐 때, THEN THE `Daily_Snapshot` SHALL 오류를 기록하고 커밋 없이 종료한다.
7. THE `Daily_Snapshot` SHALL Git 커밋 실행 결과(성공/실패, 커밋 해시)를 에이전트 관찰 가능성 시스템을 통해 기록한다.
8. WHERE `[ingestion.git_snapshot]` 섹션에 `enabled = false`가 설정된 경우, THE `Daily_Snapshot` SHALL 일일 커밋 작업을 등록하지 않는다.
9. WHERE `[ingestion.git_snapshot]` 섹션에 `schedule` 키가 설정된 경우, THE `Daily_Snapshot` SHALL 기본 자정 스케줄 대신 해당 cron 표현식을 사용한다.

---

### 요구사항 12: 에이전트 컨텍스트 활용

**사용자 스토리:** 에이전트로서, 대화 중 사용자의 질문과 관련된 위키 내용을 자동으로 참조하여 개인화된 응답을 제공하기를 원한다. 그래야 축적된 지식이 실제 대화에서 활용되기 때문이다.

#### 인수 기준

1. WHEN 에이전트 루프가 사용자 메시지를 처리할 때, THE `Ingestion_Pipeline` SHALL `WikiMemory.recall(query, limit)` 인터페이스를 통해 관련 위키 섹션을 검색하여 LLM 컨텍스트에 포함한다.
2. THE `Ingestion_Pipeline` SHALL 위키 recall 결과를 시스템 프롬프트 또는 컨텍스트 메시지로 에이전트 루프에 주입하는 방식을 `naraeclaw-config`의 `[ingestion.context]` 섹션에서 설정 가능하게 한다.
3. THE `Ingestion_Pipeline` SHALL recall 시 반환되는 위키 섹션 수를 `[ingestion.context]` 섹션의 `max_recall_sections` 키로 제한한다. 기본값은 5이다.
4. WHEN 위키 recall 결과가 없을 때, THE `Ingestion_Pipeline` SHALL 에이전트 루프에 빈 컨텍스트를 주입하고 정상적으로 응답을 생성한다.
5. THE `Ingestion_Pipeline` SHALL 위키 컨텍스트 주입이 에이전트 루프의 기존 메모리 recall 흐름(`crates/naraeclaw-channels/src/orchestrator/mod.rs:1844`)과 충돌하지 않도록 독립적으로 동작한다.
6. WHERE `[ingestion.context]` 섹션에 `enabled = false`가 설정된 경우, THE `Ingestion_Pipeline` SHALL 위키 컨텍스트를 에이전트 루프에 주입하지 않는다.

---

### 요구사항 13: 설정 및 인증 관리

**사용자 스토리:** 운영자로서, 각 소스의 인증 정보와 수집 범위를 `config.toml`에서 명확하게 설정하기를 원한다. 그래야 보안을 유지하면서 수집 동작을 제어할 수 있기 때문이다.

#### 인수 기준

1. THE `Ingestion_Config` SHALL `naraeclaw-config`의 TOML 설정에서 `[ingestion]` 최상위 섹션으로 정의된다.
2. THE `Ingestion_Config` SHALL 각 소스를 `[ingestion.sources.{source_id}]` 형식의 하위 섹션으로 정의하며, `type` 키로 소스 유형(`local`, `slack`, `notion`, `gdrive`, `email`, `confluence`, `generic_http`)을 지정한다.
3. THE `Ingestion_Config` SHALL 소스별 인증 정보(API 키, 토큰, 비밀번호)를 `naraeclaw-config`의 `secrets` 섹션 또는 환경 변수 참조(`${ENV_VAR}`)로 설정할 수 있게 한다.
4. THE `Ingestion_Config` SHALL 각 소스에 `enabled` 키를 지원하며, `enabled = false`인 소스는 수동 요청 및 자동 스케줄 모두에서 건너뛴다.
5. THE `Ingestion_Config` SHALL 각 소스에 `max_items` 키를 지원하여 단일 수집 작업에서 처리할 최대 항목 수를 제한한다.
6. THE `Ingestion_Config` SHALL 각 소스에 `wiki_category` 키를 지원하여 수집 결과가 저장될 위키 카테고리를 명시적으로 지정할 수 있게 한다.
7. IF 필수 인증 정보가 누락된 소스가 있을 때, THEN THE `Ingestion_Config` SHALL 에이전트 시작 시 경고를 출력하고 해당 소스를 비활성화한다.
8. THE `Ingestion_Config` SHALL `naraeclaw-config`의 `Configurable` derive 매크로를 사용하여 설정 스키마를 자동 생성한다.

---

### 요구사항 14: 수집 범위 제한 및 보안

**사용자 스토리:** 사용자로서, 에이전트가 수집할 수 있는 소스와 범위를 명확히 제한하기를 원한다. 그래야 의도하지 않은 데이터가 위키에 저장되는 것을 방지할 수 있기 때문이다.

#### 인수 기준

1. THE `Ingestion_Pipeline` SHALL 사용자가 명시적으로 지정하거나 `Ingestion_Config`에 등록된 소스만 수집한다. 등록되지 않은 소스에 대한 수집 요청은 거부한다.
2. THE `Ingestion_Pipeline` SHALL 로컬 파일 수집 시 `naraeclaw-runtime`의 보안 정책(`crates/naraeclaw-runtime/src/security/`)에 따라 허용된 경로 범위 내의 파일만 접근한다.
3. THE `Ingestion_Pipeline` SHALL 수집된 원본 콘텐츠를 `wiki/` 디렉토리 외부에 영구 저장하지 않는다. 처리 중 임시 파일이 필요한 경우 처리 완료 후 즉시 삭제한다.
4. THE `Ingestion_Pipeline` SHALL 각 소스의 인증 토큰을 로그나 위키 파일에 기록하지 않는다.
5. WHEN 수집 요청이 `Ingestion_Config`에 등록되지 않은 소스를 대상으로 할 때, THE `Ingestion_Pipeline` SHALL 요청을 거부하고 사용자에게 설정 추가를 안내한다.
6. THE `Ingestion_Pipeline` SHALL 위키에 저장되는 콘텐츠에서 이메일 주소, 전화번호 등 개인 식별 정보(PII)를 마스킹할지 여부를 `[ingestion]` 섹션의 `mask_pii` 키로 설정 가능하게 한다.

---

## 정확성 속성 (Property-Based Testing)

### 속성 1: 소스 커넥터 멱등성

*임의의* 소스와 콘텐츠에 대해, 동일한 `Ingestion_Job`을 두 번 실행한 결과 위키 파일 상태는 한 번 실행한 것과 동일해야 한다.

`∀ source, content: run_job(source) ; run_job(source) ≡ run_job(source)`

**검증 대상: 요구사항 1.6, 2.6, 3.6, 4.6, 5.6**

---

### 속성 2: Dedup_Hash 중복 방지

*임의의* 콘텐츠에 대해, 동일한 `Dedup_Hash`를 가진 콘텐츠는 위키에 중복 섹션을 생성하지 않아야 한다.

`∀ content: count_sections_for(hash(content)) ≤ 1`

**검증 대상: 요구사항 1.6, 5.6**

---

### 속성 3: LLM 분석 결과 구조 불변식

*임의의* 원본 콘텐츠에 대해, `LLM_Analyzer`가 반환하는 결과는 항상 유효한 마크다운 구조(하나 이상의 H2 섹션 포함)를 가져야 한다.

`∀ content: analyze(content).sections.len() ≥ 1 ∧ all_sections_have_h2_header(analyze(content))`

**검증 대상: 요구사항 7.3**

---

### 속성 4: 수집 → 저장 → recall 라운드트립

*임의의* 소스 콘텐츠에 대해, 수집 후 저장된 내용은 `WikiMemory.recall`로 검색 가능해야 한다.

`∀ content, key: ingest(content) → ∃ section ∈ recall(key): section.content ≠ ""`

**검증 대상: 요구사항 8.1, 12.1**

---

### 속성 5: 일일 스냅샷 커밋 조건

*임의의* `wiki/` 디렉토리 상태에 대해, 변경사항이 있을 때만 Git 커밋이 생성되어야 한다.

`∀ wiki_state: has_changes(wiki_state) ↔ git_commit_created(run_snapshot(wiki_state))`

**검증 대상: 요구사항 11.3, 11.4**

---

### 속성 6: 카테고리 매핑 일관성

*임의의* 소스 유형과 대상 식별자에 대해, `Ingestion_Pipeline`이 결정하는 위키 카테고리는 항상 동일한 소스 유형과 식별자에 대해 동일한 값을 반환해야 한다.

`∀ source_type, target_id: category_for(source_type, target_id) = category_for(source_type, target_id)`

**검증 대상: 요구사항 8.2**

---

### 속성 7: 수집 상태 파일 단조 증가

*임의의* 수집 작업 실행 후, `wiki/.ingestion-state.json`에 기록된 마지막 성공 실행 시각은 이전 값보다 항상 크거나 같아야 한다.

`∀ job: last_run_after(job) ≥ last_run_before(job)`

**검증 대상: 요구사항 8.6**

---

### 속성 8: 보안 경계 불변식

*임의의* 수집 요청에 대해, `Ingestion_Config`에 등록되지 않은 소스에 대한 수집은 항상 거부되어야 한다.

`∀ source: ¬registered(source) → rejected(ingest_request(source))`

**검증 대상: 요구사항 14.1, 14.5**
