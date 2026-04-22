# 작업 목록: 데스크탑 권한 관리 화면

## 1. 프로젝트 기반 구조 설정
- [ ] 1.1 `web/src/pages/permissions/` 디렉토리 생성 및 `types.ts` 타입 정의 파일 작성 (AutonomyLevel, SandboxBackend, OtpMethod, RiskLevel, AutonomyConfig, SecurityConfig 등 모든 인터페이스)
- [ ] 1.2 `web/src/pages/permissions/controls/ChipList.tsx` 편집 가능한 칩 목록 컴포넌트 구현 (추가 입력 필드, 삭제 버튼, chipColor 콜백, disabled 상태, ARIA 레이블)
- [ ] 1.3 `web/src/pages/permissions/controls/SegmentSlider.tsx` 3단계 세그먼트 선택기 컴포넌트 구현 (options 배열, 색상 코딩, 키보드 화살표 키 지원, ARIA 레이블)
- [ ] 1.4 `web/src/pages/permissions/controls/RiskBanner.tsx` 위험 수준 배너 컴포넌트 구현 (safe/caution/danger 색상, 요인 목록 펼침, aria-live 영역)

## 2. 순수 함수 및 상태 관리
- [ ] 2.1 `web/src/pages/permissions/riskLevel.ts` 위험 수준 계산 순수 함수 구현 (`computeRiskLevel`, `formatCentsToDollars`, `getToolChipColor`, `shouldShowRateWarning`)
- [ ] 2.2 `web/src/pages/permissions/usePermissionsState.ts` 상태 관리 훅 구현 (getConfig → 파싱 → autonomy/security 추출, updateAutonomy/updateSecurity, save/reset, dirty 플래그, savedState 관리)

## 3. 카드 섹션 컴포넌트 구현
- [ ] 3.1 `AutonomyCard.tsx` 구현 (SegmentSlider로 AutonomyLevel 제어, workspace_only/require_approval_for_medium_risk/block_high_risk_commands 토글, shell_timeout_secs 숫자 입력, full 레벨 확인 다이얼로그, 카드별 Risk_Indicator)
- [ ] 3.2 `ShellCard.tsx` 구현 (allowed_commands/forbidden_paths/allowed_roots/shell_env_passthrough 각각 ChipList로 표시)
- [ ] 3.3 `ToolApprovalCard.tsx` 구현 (auto_approve/always_ask ChipList, 칩 간 이동 기능, non_cli_excluded_tools ChipList, 승인 상태별 칩 색상)
- [ ] 3.4 `RateLimitCard.tsx` 구현 (max_actions_per_hour 슬라이더 0~500 + 숫자 입력, max_cost_per_day_cents 슬라이더 0~10000 + 숫자 입력 + 달러 변환 레이블, 임계값 초과 경고)
- [ ] 3.5 `SandboxCard.tsx` 구현 (sandbox.enabled 토글, backend 드롭다운, firejail 조건부 firejail_args 필드, resources 슬라이더들, memory_monitoring 토글, backend=none 경고)
- [ ] 3.6 `OtpCard.tsx` 구현 (otp.enabled 토글, enabled=false 시 하위 필드 비활성화, method 드롭다운, gated_actions/gated_domains/gated_domain_categories ChipList, 숫자 입력 필드들)
- [ ] 3.7 `EStopCard.tsx` 구현 (estop.enabled 토글, 빨간색 긴급 정지 버튼 + 확인 다이얼로그, 상태 배지, require_otp_to_resume 토글, state_file 읽기 전용 표시)
- [ ] 3.8 `AuditCard.tsx` 구현 (audit.enabled 토글, log_path 텍스트 입력, max_size_mb 슬라이더 10~1000, sign_events 토글, 로그 뷰어 영역, 비활성화 시 뷰어 비활성화)

## 4. 메인 페이지 및 라우팅 통합
- [ ] 4.1 `web/src/pages/permissions/Permissions.tsx` 메인 페이지 컴포넌트 구현 (RiskBanner + 저장/초기화 버튼 헤더, 섹션 네비게이션 사이드바, 카드 섹션 스크롤 영역, 미저장 변경 배지, 성공/오류 알림)
- [ ] 4.2 `web/src/App.tsx`에 `/permissions` 라우트 추가 및 `web/src/components/layout/Sidebar.tsx`에 네비게이션 항목 추가
- [ ] 4.3 `web/src/lib/i18n.ts`에 권한 관리 페이지 관련 i18n 키 추가 (nav.permissions, permissions.* 네임스페이스)
- [ ] 4.4 페이지 이탈 시 미저장 변경 확인 다이얼로그 구현 (beforeunload 이벤트 + React Router 네비게이션 가드)

## 5. 속성 기반 테스트 (fast-check)
- [ ] 5.1 `web/src/pages/permissions/__tests__/chipList.property.test.ts` — Property 1: 칩 목록 추가/제거 불변성 테스트 (fast-check, 100회 반복)
- [ ] 5.2 `web/src/pages/permissions/__tests__/toolMovement.property.test.ts` — Property 2: 도구 이동 보존 속성 테스트
- [ ] 5.3 `web/src/pages/permissions/__tests__/toolChipColor.property.test.ts` — Property 3: 도구 승인 상태 색상 매핑 테스트
- [ ] 5.4 `web/src/pages/permissions/__tests__/formatCents.property.test.ts` — Property 4: 센트-달러 변환 테스트
- [ ] 5.5 `web/src/pages/permissions/__tests__/rateWarning.property.test.ts` — Property 5: 속도/비용 임계값 경고 테스트
- [ ] 5.6 `web/src/pages/permissions/__tests__/riskLevel.property.test.ts` — Property 6: 위험 수준 계산 결정성 테스트
- [ ] 5.7 `web/src/pages/permissions/__tests__/resetState.property.test.ts` — Property 7: 설정 초기화 복원 테스트
- [ ] 5.8 `web/src/pages/permissions/__tests__/dirtyFlag.property.test.ts` — Property 8: 더티 플래그 추적 테스트
- [ ] 5.9 `web/src/pages/permissions/__tests__/serialization.property.test.ts` — Property 9: 설정 직렬화 라운드트립 테스트

## 6. 단위 및 통합 테스트
- [ ] 6.1 각 카드 섹션 컴포넌트 렌더링 단위 테스트 (올바른 컨트롤 렌더링, 조건부 표시, 비활성화 상태)
- [ ] 6.2 `usePermissionsState` 훅 단위 테스트 (로드, 업데이트, 저장, 리셋, 오류 처리)
- [ ] 6.3 Gateway API 모킹 통합 테스트 (저장/로드 플로우, 오류 응답 처리)
