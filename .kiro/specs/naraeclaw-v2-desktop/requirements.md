# 요구사항 문서

## 소개

NaraeClaw V2 데스크탑 앱 완성 스펙이다. 현재 Tauri 2.0 + React 19 + Axum Gateway 골격은 구현되어 있으며, Gateway sidecar 번들링(A)과 창 기본 표시(B)는 완료된 상태다. 이 문서는 미완성 항목인 **C(한국어 UI 마무리)**, **D(시스템 알림)**, **E(브랜딩 마무리)**, **F(파일 드롭·클립보드)** 네 가지 기능을 완성하기 위한 요구사항을 정의한다.

아키텍처 요약:
```
사용자 → Tauri 트레이 앱 (WebView) ──HTTP/WS──→ Gateway sidecar (Axum + 에이전트 런타임)
```

---

## 용어 정의

- **Desktop_App**: Tauri 2.0 기반 NaraeClaw 데스크탑 애플리케이션 (`apps/tauri/`)
- **WebView**: Desktop_App 내부에서 React 프론트엔드를 렌더링하는 Tauri WebviewWindow
- **Gateway**: Axum 기반 HTTP/WebSocket 서버 (`crates/naraeclaw-gateway/`), Desktop_App이 sidecar로 실행
- **I18n_Module**: 다국어 번역 딕셔너리 및 로케일 전환 로직 (`web/src/lib/i18n.ts`)
- **Tray_Menu**: 시스템 트레이 아이콘에 연결된 컨텍스트 메뉴 (`apps/tauri/src/tray/`)
- **Notification_Plugin**: `tauri-plugin-notification` — 네이티브 OS 알림 발송 플러그인
- **About_Window**: 앱 버전·라이선스·저작권 정보를 표시하는 별도 창
- **Drop_Plugin**: `tauri-plugin-drag-drop` 또는 Tauri 2.0 내장 파일 드롭 이벤트 처리
- **Clipboard_Plugin**: `tauri-plugin-clipboard-manager` — 클립보드 읽기/쓰기 플러그인
- **Telegram_Channel**: `crates/naraeclaw-channels/src/telegram.rs` 기반 Telegram 메시지 수신 채널
- **Store_Plugin**: `tauri-plugin-store` — 앱 설정·상태 영속 저장소 (`naraeclaw.json`)

---

## 요구사항

### 요구사항 1: 한국어 UI 완전 적용 (C)

**사용자 스토리:** 한국어 사용자로서, 앱의 모든 UI 텍스트가 한국어로 표시되기를 원한다. 그래야 영어를 몰라도 NaraeClaw를 불편 없이 사용할 수 있다.

#### 수용 기준

1. THE I18n_Module SHALL `ko`를 기본 로케일로 사용하며, 앱 최초 실행 시 저장된 로케일 설정이 없으면 `ko`를 적용한다.
2. WHEN 사용자가 언어 전환 드롭다운에서 로케일을 선택하면, THE WebView SHALL 페이지 새로고침 없이 즉시 해당 로케일로 전체 UI를 갱신한다.
3. THE I18n_Module SHALL `web/src/lib/i18n.ts`의 `ko` 딕셔너리에 정의된 모든 번역 키에 대해 번역 값을 반환한다.
4. WHEN `t(key)` 함수가 현재 로케일에 존재하지 않는 키로 호출되면, THE I18n_Module SHALL `en` 로케일의 동일 키 값을 폴백으로 반환한다.
5. THE Tray_Menu SHALL 트레이 메뉴 항목 텍스트("Show Dashboard", "Agent Chat", "Quit NaraeClaw")를 한국어("대시보드 열기", "에이전트 채팅", "NaraeClaw 종료")로 표시한다.
6. WHEN 사용자가 로케일을 변경하면, THE Store_Plugin SHALL 선택된 로케일 코드를 `naraeclaw.json`에 저장하여 다음 앱 실행 시 복원한다.
7. THE WebView SHALL 페어링 화면, 오류 경계 화면, 로딩 스피너 화면을 포함한 모든 전환 화면에서 현재 로케일의 번역 텍스트를 표시한다.

---

### 요구사항 2: 시스템 알림 (D)

**사용자 스토리:** NaraeClaw 사용자로서, Telegram 메시지가 수신되었을 때 네이티브 OS 알림을 받고 싶다. 그래야 앱 창이 닫혀 있거나 다른 작업 중에도 새 메시지를 놓치지 않을 수 있다.

#### 수용 기준

1. WHEN Gateway가 Telegram_Channel로부터 새 메시지 이벤트를 수신하면, THE Desktop_App SHALL Notification_Plugin을 통해 네이티브 OS 알림을 발송한다.
2. THE Desktop_App SHALL 알림 발송 전 OS 알림 권한이 부여되어 있는지 확인하고, 권한이 없으면 사용자에게 권한 요청 다이얼로그를 표시한다.
3. THE Desktop_App SHALL 알림 제목을 "NaraeClaw"로, 알림 본문을 수신된 메시지 텍스트의 앞 100자로 설정한다.
4. IF 메시지 텍스트가 100자를 초과하면, THEN THE Desktop_App SHALL 알림 본문을 100자로 잘라내고 "…"를 붙여 표시한다.
5. WHEN 사용자가 알림을 클릭하면, THE Desktop_App SHALL WebView 창을 포그라운드로 가져오고 에이전트 채팅 페이지(`/agent`)로 이동한다.
6. WHILE Desktop_App이 실행 중이고 WebView 창이 포커스 상태이면, THE Desktop_App SHALL 중복 알림 발송을 생략한다.
7. IF Notification_Plugin 초기화에 실패하면, THEN THE Desktop_App SHALL 오류를 로그에 기록하고 알림 없이 정상 동작을 계속한다.
8. THE Desktop_App SHALL macOS, Windows, Linux 세 플랫폼에서 네이티브 알림을 발송한다.

---

### 요구사항 3: 앱 브랜딩 마무리 (E)

**사용자 스토리:** NaraeClaw 사용자로서, 앱 전반에서 일관된 NaraeClaw 브랜딩을 경험하고 싶다. 그래야 앱이 완성된 제품처럼 느껴진다.

#### 수용 기준

1. THE Desktop_App SHALL Tray_Menu에 "NaraeClaw 정보" 메뉴 항목을 포함하며, 해당 항목 클릭 시 About_Window를 표시한다.
2. THE About_Window SHALL 앱 이름("NaraeClaw"), 버전(`tauri.conf.json`의 `version` 필드 값), 저작권 표기, 라이선스 정보(Apache-2.0 / MIT 이중 라이선스)를 표시한다.
3. THE About_Window SHALL 창 크기를 너비 400px, 높이 300px로 고정하며 리사이즈를 허용하지 않는다.
4. THE WebView SHALL 사이드바 로고 영역, 페어링 화면, 브라우저 탭 제목에서 "NaraeClaw" 이름과 앱 아이콘을 일관되게 표시한다.
5. THE Desktop_App SHALL `tauri.conf.json`의 `productName`이 "NaraeClaw"이고 `identifier`가 "ai.naraeclaw.desktop"임을 유지한다.
6. THE WebView SHALL 웹 UI 내에 업스트림 원본 프로젝트 이름이나 브랜딩 잔재가 노출되지 않도록 한다.
7. WHEN About_Window가 이미 열려 있는 상태에서 "NaraeClaw 정보" 메뉴를 다시 클릭하면, THE Desktop_App SHALL 새 창을 추가로 열지 않고 기존 About_Window를 포그라운드로 가져온다.

---

### 요구사항 4: 파일 드롭 및 클립보드 지원 (F)

**사용자 스토리:** NaraeClaw 사용자로서, 파일을 앱 창에 드래그 앤 드롭하거나 클립보드에서 붙여넣기하여 에이전트에게 첨부파일로 전달하고 싶다. 그래야 파일 경로를 직접 입력하지 않아도 된다.

#### 수용 기준

1. WHEN 사용자가 하나 이상의 파일을 WebView 창 위로 드래그 앤 드롭하면, THE Desktop_App SHALL 드롭된 파일의 절대 경로 목록을 에이전트 채팅 입력창에 삽입한다.
2. THE Desktop_App SHALL 드롭 이벤트를 에이전트 채팅 페이지(`/agent`)에서만 처리하며, 다른 페이지에서는 기본 브라우저 드롭 동작을 방지한다.
3. WHEN 사용자가 에이전트 채팅 입력창에 포커스가 있는 상태에서 클립보드 붙여넣기(Ctrl+V / Cmd+V)를 실행하면, THE Desktop_App SHALL 클립보드에 이미지 데이터가 있으면 임시 파일로 저장하고 해당 경로를 입력창에 삽입한다.
4. IF 드롭된 파일이 존재하지 않거나 읽기 권한이 없으면, THEN THE Desktop_App SHALL 오류 메시지를 입력창 위에 토스트 알림으로 표시한다.
5. THE Desktop_App SHALL 단일 드롭 이벤트에서 최대 10개의 파일 경로를 처리하며, 10개를 초과하는 파일은 무시하고 사용자에게 초과 사실을 알린다.
6. WHEN 파일 경로가 입력창에 삽입되면, THE WebView SHALL 기존 입력 텍스트를 보존하고 파일 경로를 줄바꿈으로 구분하여 추가한다.
7. THE Desktop_App SHALL `tauri.conf.json`의 capabilities에 `dragDrop` 및 `clipboard` 권한을 선언한다.
