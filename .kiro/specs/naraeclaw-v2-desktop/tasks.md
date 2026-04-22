# 구현 계획: NaraeClaw V2 데스크탑 완성

## 개요

NaraeClaw V2 데스크탑 앱의 미완성 4개 기능(C: 한국어 UI, D: 시스템 알림, E: 브랜딩 마무리, F: 파일 드롭·클립보드)을 구현한다. Tauri 2.0 (Rust) 백엔드와 React 19 (TypeScript) 프론트엔드를 대상으로 하며, 기존 아키텍처를 변경하지 않고 확장한다.

## Tasks

- [ ] 1. 한국어 UI 완전 적용 (C)
  - [ ] 1.1 `web/src/lib/i18n.ts`의 `ko` 딕셔너리 누락 키 보충
    - `en` 딕셔너리의 모든 키를 순회하여 `ko` 딕셔너리에 누락된 키를 식별
    - 누락된 키에 대해 한국어 번역 값을 추가
    - `ko` 딕셔너리와 `en` 딕셔너리의 키 집합이 동일한지 검증
    - _Requirements: 1.3, 1.7_

  - [ ]* 1.2 ko 딕셔너리 완전성 속성 테스트 작성
    - **Property 1: ko 딕셔너리 완전성**
    - `en` 딕셔너리의 모든 키가 `ko` 딕셔너리에도 존재하며, 빈 문자열이 아닌 번역 값을 반환하는지 검증
    - fast-check 사용
    - **Validates: Requirements 1.3**

  - [ ]* 1.3 en 폴백 메커니즘 속성 테스트 작성
    - **Property 2: en 폴백 메커니즘**
    - 현재 로케일에 없는 키로 `t()` 호출 시 `en` 값을 반환하는지 검증
    - fast-check 사용
    - **Validates: Requirements 1.4**

  - [ ] 1.4 트레이 메뉴 한국어화
    - `apps/tauri/src/tray/menu.rs`의 메뉴 텍스트를 한국어로 변경
    - "Show Dashboard" → "대시보드 열기", "Agent Chat" → "에이전트 채팅", "Status: Checking..." → "상태: 확인 중...", "Quit NaraeClaw" → "NaraeClaw 종료"
    - _Requirements: 1.5_

  - [ ] 1.5 로케일 영속 저장 확인 및 보완
    - `web/src/contexts/ThemeContext.tsx`의 `loadLocale()` / `saveLocale()` 함수가 `localStorage`에 로케일을 저장·복원하는지 확인
    - 기본 로케일이 `ko`인지 확인
    - _Requirements: 1.1, 1.6_

  - [ ]* 1.6 로케일 영속 저장 라운드트립 속성 테스트 작성
    - **Property 3: 로케일 영속 저장 라운드트립**
    - `saveLocale(locale)` 후 `loadLocale()` 호출 시 동일한 값을 반환하는지 검증
    - fast-check 사용
    - **Validates: Requirements 1.6**

- [ ] 2. 체크포인트 — 한국어 UI 검증
  - 모든 테스트가 통과하는지 확인하고, 질문이 있으면 사용자에게 문의한다.

- [ ] 3. 시스템 알림 구현 (D)
  - [ ] 3.1 Tauri 알림 플러그인 의존성 추가 및 초기화
    - `apps/tauri/Cargo.toml`에 `tauri-plugin-notification = "2.2"` 추가
    - `apps/tauri/src/lib.rs`의 `tauri::Builder`에 `.plugin(tauri_plugin_notification::init())` 추가
    - `apps/tauri/capabilities/desktop.json`에 `"notification:default"` 권한 추가
    - _Requirements: 2.1, 2.2, 2.7, 2.8_

  - [ ] 3.2 알림 모듈 구현 (`apps/tauri/src/notifications.rs`)
    - `truncate_notification_body(text, max_chars)` 함수 구현: 100자 초과 시 잘라내고 "…" 추가
    - `spawn_notification_listener(app, state)` 함수 구현: Gateway SSE `/api/events` 구독
    - 메시지 이벤트 수신 시 창 포커스 상태 확인 (`window.is_focused()`)
    - 포커스 상태가 아닐 때만 `tauri-plugin-notification`으로 네이티브 알림 발송
    - 알림 제목: "NaraeClaw", 본문: 메시지 앞 100자
    - `apps/tauri/src/lib.rs`에 `pub mod notifications;` 추가 및 `setup` 내에서 리스너 시작
    - _Requirements: 2.1, 2.3, 2.4, 2.6_

  - [ ]* 3.3 알림 본문 잘라내기 속성 테스트 작성
    - **Property 4: 알림 본문 잘라내기**
    - proptest 사용, `truncate_notification_body` 함수에 대해:
      - 100자 이하 입력 → 원본과 동일한 결과
      - 100자 초과 입력 → 정확히 100자 + "…", 앞 100자가 원본과 동일
    - **Validates: Requirements 2.3, 2.4**

  - [ ] 3.4 알림 클릭 시 창 포커스 및 페이지 이동 구현
    - 알림 클릭 이벤트 핸들러에서 `show_main_window(app, Some("/agent"))` 호출
    - 기존 `events.rs`의 `show_main_window` 함수를 `pub`으로 변경하여 재사용
    - _Requirements: 2.5_

- [ ] 4. 체크포인트 — 시스템 알림 검증
  - 모든 테스트가 통과하는지 확인하고, 질문이 있으면 사용자에게 문의한다.

- [ ] 5. 앱 브랜딩 마무리 (E)
  - [ ] 5.1 트레이 메뉴에 "NaraeClaw 정보" 항목 추가
    - `apps/tauri/src/tray/menu.rs`에 `about` 메뉴 항목 추가 ("NaraeClaw 정보")
    - `quit` 앞, 구분선 뒤에 삽입
    - _Requirements: 3.1_

  - [ ] 5.2 About Window 구현
    - `apps/tauri/src/tray/events.rs`에 `show_about_window` 함수 구현
    - `WebviewWindowBuilder`로 별도 창 생성: 400x300, 리사이즈 불가
    - 인라인 HTML로 앱 이름("NaraeClaw"), 버전(`tauri.conf.json` version), 저작권, 라이선스(Apache-2.0 / MIT) 표시
    - 이미 열려 있으면 기존 창에 포커스만 이동 (싱글턴 패턴)
    - `events.rs`의 `handle_menu_event`에 `"about"` 매치 암 추가
    - _Requirements: 3.1, 3.2, 3.3, 3.7_

  - [ ] 5.3 브랜딩 일관성 검증 및 수정
    - `tauri.conf.json`의 `productName: "NaraeClaw"`, `identifier: "ai.naraeclaw.desktop"` 확인
    - `web/src/index.html`의 `<title>` 태그가 "NaraeClaw"인지 확인 및 수정
    - 사이드바, 페어링 화면 등에서 업스트림 브랜딩 잔재가 없는지 확인
    - _Requirements: 3.4, 3.5, 3.6_

- [ ] 6. 파일 드롭 및 클립보드 지원 (F)
  - [ ] 6.1 클립보드 플러그인 의존성 추가 및 Capabilities 설정
    - `apps/tauri/Cargo.toml`에 `tauri-plugin-clipboard-manager = "2.2"` 추가
    - `apps/tauri/src/lib.rs`의 `tauri::Builder`에 `.plugin(tauri_plugin_clipboard_manager::init())` 추가
    - `apps/tauri/capabilities/desktop.json`에 `"clipboard-manager:allow-read-image"`, `"clipboard-manager:allow-write-text"` 권한 추가
    - _Requirements: 4.7_

  - [ ] 6.2 파일 드롭 이벤트 처리 구현 (Rust 사이드)
    - `apps/tauri/src/lib.rs`의 `setup` 내에서 `window.on_drag_drop_event` 핸들러 등록
    - `DragDropEvent::Drop` 시 최대 10개 파일 경로 제한
    - 파일 존재 여부 및 읽기 권한 검증 (`Path::exists()`, `Path::metadata()`)
    - 유효한 파일 경로를 `naraeclaw://file-drop` 이벤트로 프론트엔드에 전송
    - 초과 파일이 있으면 `exceeded: true` 플래그 포함
    - _Requirements: 4.1, 4.4, 4.5_

  - [ ] 6.3 프론트엔드 파일 드롭 핸들러 구현
    - `web/src/pages/AgentChat.tsx`에 `@tauri-apps/api/event`의 `listen` 사용하여 `naraeclaw://file-drop` 이벤트 리스너 추가
    - `/agent` 페이지에서만 파일 경로를 입력창에 삽입 (기존 텍스트 보존, 줄바꿈 구분)
    - 10개 초과 시 토스트 알림 표시
    - 다른 페이지에서는 기본 브라우저 드롭 동작 방지 (`e.preventDefault()`)
    - _Requirements: 4.1, 4.2, 4.5, 4.6_

  - [ ]* 6.4 파일 경로 삽입 속성 테스트 작성
    - **Property 5: 파일 경로 삽입 시 기존 텍스트 보존**
    - fast-check 사용, 기존 입력 텍스트와 1~10개 파일 경로에 대해:
      - 결과가 기존 텍스트로 시작
      - 모든 경로가 결과에 포함
      - 경로들이 줄바꿈으로 구분
    - **Validates: Requirements 4.1, 4.6**

  - [ ]* 6.5 파일 개수 제한 속성 테스트 작성
    - **Property 6: 파일 개수 제한**
    - fast-check 사용, N > 10개 파일 경로에 대해:
      - 처리 결과가 정확히 10개 경로만 포함
      - `exceeded` 플래그가 `true`
    - **Validates: Requirements 4.5**

  - [ ] 6.6 클립보드 이미지 붙여넣기 구현
    - `apps/tauri/src/commands/clipboard.rs` 신규 생성: `save_clipboard_image` Tauri 커맨드 구현
    - 바이트 배열을 임시 디렉토리(`naraeclaw/`)에 PNG 파일로 저장, 경로 반환
    - `apps/tauri/src/commands/mod.rs`에 `pub mod clipboard;` 추가
    - `apps/tauri/src/lib.rs`의 `invoke_handler`에 `commands::clipboard::save_clipboard_image` 등록
    - `web/src/pages/AgentChat.tsx`에 `onPaste` 핸들러 추가: 클립보드 이미지 감지 → Tauri invoke → 경로 삽입
    - _Requirements: 4.3_

- [ ] 7. 통합 및 최종 검증
  - [ ] 7.1 전체 기능 연결 확인
    - `apps/tauri/src/lib.rs`에서 모든 신규 모듈(`notifications`, `commands::clipboard`)이 올바르게 등록되었는지 확인
    - `tauri::Builder`의 플러그인 체인, invoke_handler, setup 내 이벤트 핸들러가 모두 연결되었는지 확인
    - `apps/tauri/capabilities/desktop.json`에 모든 필요 권한이 선언되었는지 확인
    - _Requirements: 1.1–1.7, 2.1–2.8, 3.1–3.7, 4.1–4.7_

  - [ ]* 7.2 통합 테스트 작성
    - 알림 파이프라인: Mock SSE 이벤트 → 알림 발송 확인
    - 클립보드 이미지: Mock 바이트 배열 → 임시 파일 생성 확인
    - `tauri.conf.json` 브랜딩 값 검증 (productName, identifier)
    - _Requirements: 2.1, 3.5, 4.3_

- [ ] 8. 최종 체크포인트 — 전체 테스트 통과 확인
  - 모든 테스트가 통과하는지 확인하고, 질문이 있으면 사용자에게 문의한다.

## 참고

- `*` 표시된 태스크는 선택적이며 빠른 MVP를 위해 건너뛸 수 있음
- 각 태스크는 추적 가능성을 위해 특정 요구사항을 참조함
- 체크포인트는 점진적 검증을 보장함
- 속성 테스트는 보편적 정확성 속성을 검증하고, 단위 테스트는 특정 예시와 엣지 케이스를 검증함
- Rust 측 속성 테스트는 `proptest`, TypeScript 측은 `fast-check` 사용
