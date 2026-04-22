# 요구사항 문서: 데스크탑 권한 관리 화면

## 소개

NaraeClaw 데스크탑 앱(Tauri 2.0 + React 19)에 에이전트의 자율성, 보안, 감사 설정을 시각적으로 제어할 수 있는 전용 권한 관리 화면을 추가한다. 기존 `Config` 페이지의 TOML 편집 방식과 달리, 카드 기반 레이아웃과 인터랙티브 컨트롤(슬라이더, 토글, 칩)을 사용하여 Dribbble 스타일의 모던 대시보드 UX를 제공한다. 모든 설정 변경은 Gateway API(`PUT /api/config`)를 통해 저장되며, 위험 수준별 색상 코딩으로 현재 보안 상태를 직관적으로 파악할 수 있다.

## 용어집

- **Permissions_UI**: 에이전트 권한 및 보안 설정을 시각적으로 제어하는 전용 React 페이지 컴포넌트
- **Autonomy_Card**: 자율성 레벨(read_only, supervised, full)을 제어하는 카드 UI 섹션
- **Shell_Card**: 셸 명령 허용 목록(allowed_commands)과 경로 차단 목록(forbidden_paths)을 관리하는 카드 UI 섹션
- **Tool_Approval_Card**: 도구 자동 승인(auto_approve) 및 항상 승인 필요(always_ask) 목록을 관리하는 카드 UI 섹션
- **Rate_Limit_Card**: 시간당 최대 작업 수(max_actions_per_hour)와 일일 비용 한도(max_cost_per_day_cents)를 제어하는 카드 UI 섹션
- **Sandbox_Card**: 샌드박스 백엔드 선택 및 리소스 제한을 설정하는 카드 UI 섹션
- **OTP_Card**: OTP 게이팅 활성화, 게이트된 작업/도메인을 관리하는 카드 UI 섹션
- **EStop_Card**: 긴급 정지(Emergency Stop) 버튼과 상태를 표시하는 카드 UI 섹션
- **Audit_Card**: 감사 로그 설정 및 실시간 로그 뷰어를 제공하는 카드 UI 섹션
- **Gateway_API**: NaraeClaw Gateway 서버의 REST API (`GET/PUT /api/config`)
- **Risk_Indicator**: 현재 설정의 위험 수준을 색상으로 표시하는 시각적 요소 (녹색=안전, 노란색=주의, 빨간색=위험)
- **Config_Snapshot**: 설정 변경 전 상태를 저장하여 복원할 수 있는 스냅샷 객체
- **AutonomyLevel**: 에이전트 자율성 수준 열거형 (`read_only`, `supervised`, `full`)
- **SandboxBackend**: 샌드박스 백엔드 열거형 (`auto`, `landlock`, `firejail`, `bubblewrap`, `docker`, `sandbox-exec`, `none`)

## 요구사항

### 요구사항 1: 권한 관리 페이지 라우팅 및 레이아웃

**사용자 스토리:** 사용자로서, 전용 권한 관리 화면에 접근하여 에이전트의 모든 보안 설정을 한 곳에서 관리하고 싶다.

#### 수용 기준

1. THE Permissions_UI SHALL 기존 Layout 컴포넌트 내에서 `/permissions` 경로로 접근 가능한 독립 페이지로 렌더링한다.
2. THE Permissions_UI SHALL 좌측 사이드바 네비게이션에 각 카드 섹션(Autonomy, Shell, Tool Approval, Rate Limit, Sandbox, OTP, E-Stop, Audit)으로 스크롤하는 링크를 제공한다.
3. THE Permissions_UI SHALL 기존 `--pc-*` CSS 변수 시스템을 사용하여 다크 모드 우선 테마를 적용한다.
4. THE Permissions_UI SHALL 최소 너비 600px부터 최대 7680px까지 반응형 레이아웃을 지원한다.
5. THE Permissions_UI SHALL 페이지 상단에 현재 전체 보안 상태를 요약하는 Risk_Indicator 배너를 표시한다.

### 요구사항 2: 자율성 레벨 제어

**사용자 스토리:** 사용자로서, 에이전트의 자율성 레벨을 시각적 슬라이더로 직관적으로 변경하고 싶다.

#### 수용 기준

1. THE Autonomy_Card SHALL 3단계 세그먼트 슬라이더(read_only, supervised, full)로 현재 AutonomyLevel을 표시하고 변경할 수 있게 한다.
2. WHEN 사용자가 AutonomyLevel을 `full`로 변경하면, THE Autonomy_Card SHALL 빨간색 경고 배지와 확인 다이얼로그를 표시한다.
3. THE Autonomy_Card SHALL `workspace_only` 토글 스위치를 제공한다.
4. THE Autonomy_Card SHALL `require_approval_for_medium_risk` 토글 스위치를 제공한다.
5. THE Autonomy_Card SHALL `block_high_risk_commands` 토글 스위치를 제공한다.
6. THE Autonomy_Card SHALL `shell_timeout_secs` 값을 숫자 입력 필드로 편집할 수 있게 한다.
7. WHEN AutonomyLevel이 `read_only`이면, THE Autonomy_Card SHALL 녹색 Risk_Indicator를 표시한다.
8. WHEN AutonomyLevel이 `supervised`이면, THE Autonomy_Card SHALL 노란색 Risk_Indicator를 표시한다.
9. WHEN AutonomyLevel이 `full`이면, THE Autonomy_Card SHALL 빨간색 Risk_Indicator를 표시한다.

### 요구사항 3: 셸 명령 허용/차단 관리

**사용자 스토리:** 사용자로서, 에이전트가 실행할 수 있는 셸 명령과 접근 차단 경로를 편집 가능한 목록으로 관리하고 싶다.

#### 수용 기준

1. THE Shell_Card SHALL `allowed_commands` 목록을 편집 가능한 칩(chip) 목록으로 표시한다.
2. THE Shell_Card SHALL 칩 목록에 새 명령을 추가하는 텍스트 입력 필드와 추가 버튼을 제공한다.
3. WHEN 사용자가 칩의 삭제 아이콘을 클릭하면, THE Shell_Card SHALL 해당 명령을 `allowed_commands` 목록에서 제거한다.
4. THE Shell_Card SHALL `forbidden_paths` 목록을 편집 가능한 칩 목록으로 표시한다.
5. THE Shell_Card SHALL `forbidden_paths`에 새 경로를 추가하는 텍스트 입력 필드와 추가 버튼을 제공한다.
6. WHEN 사용자가 `forbidden_paths` 칩의 삭제 아이콘을 클릭하면, THE Shell_Card SHALL 해당 경로를 목록에서 제거한다.
7. THE Shell_Card SHALL `allowed_roots` 목록을 편집 가능한 칩 목록으로 표시한다.
8. THE Shell_Card SHALL `shell_env_passthrough` 목록을 편집 가능한 칩 목록으로 표시한다.

### 요구사항 4: 도구 승인 제어

**사용자 스토리:** 사용자로서, 어떤 도구가 자동 승인되고 어떤 도구가 항상 승인을 요구하는지 시각적으로 관리하고 싶다.

#### 수용 기준

1. THE Tool_Approval_Card SHALL `auto_approve` 목록을 편집 가능한 칩 목록으로 표시한다.
2. THE Tool_Approval_Card SHALL `always_ask` 목록을 편집 가능한 칩 목록으로 표시한다.
3. WHEN 사용자가 `auto_approve` 칩을 `always_ask` 영역으로 이동하면, THE Tool_Approval_Card SHALL 해당 도구를 `auto_approve`에서 제거하고 `always_ask`에 추가한다.
4. WHEN 사용자가 `always_ask` 칩을 `auto_approve` 영역으로 이동하면, THE Tool_Approval_Card SHALL 해당 도구를 `always_ask`에서 제거하고 `auto_approve`에 추가한다.
5. THE Tool_Approval_Card SHALL `non_cli_excluded_tools` 목록을 편집 가능한 칩 목록으로 표시한다.
6. THE Tool_Approval_Card SHALL 각 도구 칩에 해당 도구의 승인 상태를 색상으로 구분하여 표시한다 (녹색=자동 승인, 빨간색=항상 승인 필요, 회색=기본).

### 요구사항 5: 속도 및 비용 제한

**사용자 스토리:** 사용자로서, 에이전트의 시간당 작업 수와 일일 비용 한도를 슬라이더로 조절하고 싶다.

#### 수용 기준

1. THE Rate_Limit_Card SHALL `max_actions_per_hour` 값을 범위 슬라이더(0~500)와 숫자 입력 필드로 표시하고 변경할 수 있게 한다.
2. THE Rate_Limit_Card SHALL `max_cost_per_day_cents` 값을 범위 슬라이더(0~10000)와 숫자 입력 필드로 표시하고 변경할 수 있게 한다.
3. THE Rate_Limit_Card SHALL `max_cost_per_day_cents` 값을 달러 단위로 변환하여 레이블에 표시한다 (예: 500 cents → $5.00).
4. WHEN `max_actions_per_hour` 값이 100을 초과하면, THE Rate_Limit_Card SHALL 노란색 경고 레이블을 표시한다.
5. WHEN `max_cost_per_day_cents` 값이 2000을 초과하면, THE Rate_Limit_Card SHALL 노란색 경고 레이블을 표시한다.

### 요구사항 6: 샌드박스 설정

**사용자 스토리:** 사용자로서, 샌드박스 백엔드를 선택하고 리소스 제한을 설정하고 싶다.

#### 수용 기준

1. THE Sandbox_Card SHALL `sandbox.enabled` 토글 스위치를 제공한다.
2. THE Sandbox_Card SHALL `sandbox.backend` 값을 드롭다운 선택기(auto, landlock, firejail, bubblewrap, docker, sandbox-exec, none)로 표시하고 변경할 수 있게 한다.
3. WHEN `sandbox.backend`이 `firejail`이면, THE Sandbox_Card SHALL `firejail_args` 편집 필드를 추가로 표시한다.
4. THE Sandbox_Card SHALL `resources.max_memory_mb` 값을 범위 슬라이더(64~4096)와 숫자 입력 필드로 표시한다.
5. THE Sandbox_Card SHALL `resources.max_cpu_time_seconds` 값을 범위 슬라이더(10~600)와 숫자 입력 필드로 표시한다.
6. THE Sandbox_Card SHALL `resources.max_subprocesses` 값을 범위 슬라이더(1~100)와 숫자 입력 필드로 표시한다.
7. THE Sandbox_Card SHALL `resources.memory_monitoring` 토글 스위치를 제공한다.
8. WHEN `sandbox.backend`이 `none`이면, THE Sandbox_Card SHALL 빨간색 경고 배지를 표시한다.

### 요구사항 7: OTP 게이팅

**사용자 스토리:** 사용자로서, OTP 게이팅을 활성화하고 어떤 작업과 도메인에 OTP를 요구할지 관리하고 싶다.

#### 수용 기준

1. THE OTP_Card SHALL `otp.enabled` 토글 스위치를 제공한다.
2. WHILE `otp.enabled`가 false이면, THE OTP_Card SHALL OTP 세부 설정 필드를 비활성화(disabled) 상태로 표시한다.
3. THE OTP_Card SHALL `otp.method` 값을 드롭다운 선택기(totp, pairing, cli-prompt)로 표시한다.
4. THE OTP_Card SHALL `otp.gated_actions` 목록을 편집 가능한 칩 목록으로 표시한다.
5. THE OTP_Card SHALL `otp.gated_domains` 목록을 편집 가능한 칩 목록으로 표시한다.
6. THE OTP_Card SHALL `otp.gated_domain_categories` 목록을 편집 가능한 칩 목록으로 표시한다.
7. THE OTP_Card SHALL `otp.token_ttl_secs` 값을 숫자 입력 필드로 편집할 수 있게 한다.
8. THE OTP_Card SHALL `otp.cache_valid_secs` 값을 숫자 입력 필드로 편집할 수 있게 한다.
9. THE OTP_Card SHALL `otp.challenge_max_attempts` 값을 숫자 입력 필드로 편집할 수 있게 한다.

### 요구사항 8: 긴급 정지 (E-Stop)

**사용자 스토리:** 사용자로서, 긴급 상황에서 원클릭으로 에이전트를 즉시 정지시키고 현재 상태를 확인하고 싶다.

#### 수용 기준

1. THE EStop_Card SHALL `estop.enabled` 토글 스위치를 제공한다.
2. THE EStop_Card SHALL 크고 눈에 띄는 빨간색 긴급 정지 버튼을 표시한다.
3. WHEN 사용자가 긴급 정지 버튼을 클릭하면, THE EStop_Card SHALL 확인 다이얼로그를 표시한 후 긴급 정지를 실행한다.
4. THE EStop_Card SHALL 현재 긴급 정지 상태(활성/비활성)를 시각적 배지로 표시한다.
5. THE EStop_Card SHALL `estop.require_otp_to_resume` 토글 스위치를 제공한다.
6. THE EStop_Card SHALL `estop.state_file` 경로를 읽기 전용 텍스트로 표시한다.
7. WHEN 긴급 정지가 활성 상태이면, THE EStop_Card SHALL 페이지 상단 Risk_Indicator 배너를 빨간색 긴급 상태로 변경한다.

### 요구사항 9: 감사 로그 설정 및 뷰어

**사용자 스토리:** 사용자로서, 감사 로그 설정을 관리하고 최근 감사 로그를 실시간으로 조회하고 싶다.

#### 수용 기준

1. THE Audit_Card SHALL `audit.enabled` 토글 스위치를 제공한다.
2. THE Audit_Card SHALL `audit.log_path` 값을 텍스트 입력 필드로 편집할 수 있게 한다.
3. THE Audit_Card SHALL `audit.max_size_mb` 값을 범위 슬라이더(10~1000)와 숫자 입력 필드로 표시한다.
4. THE Audit_Card SHALL `audit.sign_events` 토글 스위치를 제공한다.
5. THE Audit_Card SHALL 최근 감사 로그 항목을 시간순 목록으로 표시하는 로그 뷰어 영역을 제공한다.
6. WHEN 감사 로그가 비활성화 상태이면, THE Audit_Card SHALL 로그 뷰어 영역을 비활성화 상태로 표시한다.

### 요구사항 10: 설정 저장 및 복원

**사용자 스토리:** 사용자로서, 권한 설정 변경 사항을 저장하고, 실수로 변경한 경우 이전 상태로 복원하고 싶다.

#### 수용 기준

1. THE Permissions_UI SHALL 페이지 상단에 "저장" 버튼을 제공하여 모든 변경 사항을 Gateway_API를 통해 저장한다.
2. WHEN 저장에 성공하면, THE Permissions_UI SHALL 녹색 성공 알림을 표시한다.
3. IF Gateway_API 저장 요청이 실패하면, THEN THE Permissions_UI SHALL 빨간색 오류 메시지와 함께 실패 원인을 표시한다.
4. THE Permissions_UI SHALL "초기화" 버튼을 제공하여 마지막 저장 상태로 모든 필드를 복원한다.
5. WHEN 저장되지 않은 변경 사항이 있으면, THE Permissions_UI SHALL "저장" 버튼을 강조 표시하고 미저장 변경 사항 배지를 표시한다.
6. WHEN 사용자가 저장되지 않은 변경 사항이 있는 상태에서 페이지를 떠나려 하면, THE Permissions_UI SHALL 확인 다이얼로그를 표시한다.

### 요구사항 11: 위험 수준 시각화

**사용자 스토리:** 사용자로서, 현재 권한 설정의 전체적인 위험 수준을 한눈에 파악하고 싶다.

#### 수용 기준

1. THE Risk_Indicator SHALL 현재 설정 조합을 분석하여 전체 위험 수준(안전, 주의, 위험)을 계산한다.
2. THE Risk_Indicator SHALL 위험 수준에 따라 색상을 적용한다 (녹색=안전, 노란색=주의, 빨간색=위험).
3. WHEN AutonomyLevel이 `full`이고 `sandbox.backend`이 `none`이면, THE Risk_Indicator SHALL 위험 수준을 "위험"으로 표시한다.
4. WHEN AutonomyLevel이 `supervised`이고 `block_high_risk_commands`가 true이면, THE Risk_Indicator SHALL 위험 수준을 "안전"으로 표시한다.
5. THE Risk_Indicator SHALL 위험 수준 계산에 기여하는 주요 요인을 툴팁 또는 펼침 영역으로 표시한다.

### 요구사항 12: 설정 직렬화 및 역직렬화 (라운드트립)

**사용자 스토리:** 개발자로서, 권한 UI에서 편집한 설정이 TOML 직렬화/역직렬화 과정에서 데이터 손실 없이 정확하게 보존되는지 보장하고 싶다.

#### 수용 기준

1. THE Permissions_UI SHALL 모든 설정 필드를 TOML 형식으로 직렬화할 때 원본 값을 정확하게 보존한다.
2. FOR ALL 유효한 AutonomyConfig 객체에 대해, TOML로 직렬화한 후 다시 역직렬화하면 THE Permissions_UI SHALL 원본과 동일한 객체를 생성한다 (라운드트립 속성).
3. FOR ALL 유효한 SecurityConfig 객체에 대해, TOML로 직렬화한 후 다시 역직렬화하면 THE Permissions_UI SHALL 원본과 동일한 객체를 생성한다 (라운드트립 속성).
4. WHEN 사용자가 칩 목록에서 항목을 추가하거나 제거한 후 저장하면, THE Permissions_UI SHALL 저장 후 다시 로드했을 때 동일한 목록을 표시한다.

### 요구사항 13: 접근성 및 키보드 네비게이션

**사용자 스토리:** 사용자로서, 키보드만으로 모든 권한 설정을 탐색하고 변경할 수 있기를 원한다.

#### 수용 기준

1. THE Permissions_UI SHALL 모든 인터랙티브 컨트롤(토글, 슬라이더, 버튼, 입력 필드)에 적절한 ARIA 레이블을 제공한다.
2. THE Permissions_UI SHALL Tab 키로 모든 인터랙티브 컨트롤 간 순차 이동을 지원한다.
3. THE Permissions_UI SHALL 슬라이더 컨트롤에서 화살표 키로 값을 조절할 수 있게 한다.
4. THE Permissions_UI SHALL 토글 스위치에서 Space 또는 Enter 키로 상태를 변경할 수 있게 한다.
5. THE Permissions_UI SHALL 위험 수준 변경 시 스크린 리더에 상태 변경을 알리는 aria-live 영역을 제공한다.
