# 구현 계획: Cloud Agent Fleet

## 개요

NaraeClaw 데스크탑(Tauri 2.0)에서 AWS, OCI, Azure 클라우드에 경량 naraeclaw 에이전트를 배포하고 원격 제어하는 기능을 구현한다. 기존 Tauri 아키텍처 패턴(`SharedState`, `GatewayClient`, `health::spawn_health_poller`)을 따르며, `apps/tauri/src/fleet/` 모듈에 핵심 로직을 배치한다.

## Tasks

- [ ] 1. Fleet 모듈 구조 및 공유 타입 정의
  - [ ] 1.1 `apps/tauri/src/fleet/types.rs` 생성 — `CloudProvider`, `AgentStatus`, `InstanceSpec`, `AgentRecord`, `SshConnectionInfo`, `SshResult` 타입 정의
    - `CloudProvider` enum에 `cli_binary()`, `display_name()`, `install_url()` 메서드 구현
    - `InstanceSpec::validate()` 메서드 구현 (필수 필드 검증)
    - `AgentRecord`에 `id`, `provider`, `region`, `instance_id`, `public_ip`, `ssh_key_path`, `ssh_user`, `ssh_port`, `status`, `error_message`, `created_at`, `last_checked_at`, `consecutive_failures`, `strict_host_key_checking`, `tags` 필드 포함
    - Serde `Serialize`/`Deserialize` derive 적용
    - _Requirements: 2.1, 2.2, 2.3, 2.7, 3.2_

  - [ ]* 1.2 `InstanceSpec::validate()` 속성 테스트 작성
    - **Property 3: InstanceSpec 유효성 검증**
    - proptest로 임의의 InstanceSpec 생성, 필수 필드 비어있으면 Err, 유효하면 Ok 확인
    - **Validates: Requirements 2.7**

  - [ ] 1.3 `apps/tauri/src/fleet/mod.rs` 생성 — fleet 모듈 루트, 하위 모듈 re-export
    - `apps/tauri/src/lib.rs`에 `pub mod fleet;` 추가
    - _Requirements: 전체 모듈 구조_

- [ ] 2. AuditLogger 및 CredentialStore 구현
  - [ ] 2.1 `apps/tauri/src/fleet/audit.rs` 생성 — `AuditLogger`, `AuditEntry`, `AuditAction` 구현
    - `AuditLogger::log()` — `~/.naraeclaw/fleet-audit.log`에 JSON 라인 형식으로 기록
    - `AuditAction` enum: `SshConnect`, `SshCommand`, `Provision`, `Install`, `Stop`, `Restart`, `Delete`, `CliDetect`, `HostKeyOverride`
    - 자격증명 값 필터링 로직 포함 (SSH 키 내용, API 토큰 등 마스킹)
    - _Requirements: 8.5, 8.6, 8.7_

  - [ ]* 2.2 로그 출력 민감 정보 필터링 속성 테스트 작성
    - **Property 10: 로그 출력 민감 정보 필터링**
    - proptest로 임의의 자격증명 문자열 포함 CLI 출력 생성, 필터링 후 자격증명 미포함 확인
    - **Validates: Requirements 1.6, 8.6, 8.7**

  - [ ] 2.3 `apps/tauri/src/fleet/credential_store.rs` 생성 — `CredentialStore` 구현
    - `load_agents()` — naraeclaw.json에서 AgentRecord 목록 로드
    - `save_agents()` — AgentRecord 목록 저장 (SSH 키 내용 미포함, 경로만 저장)
    - `ensure_permissions()` — 파일 권한 0600 확인 및 설정
    - _Requirements: 8.1, 8.2, 8.9_

  - [ ]* 2.4 저장 데이터 민감 정보 미포함 속성 테스트 작성
    - **Property 9: 저장 데이터 민감 정보 미포함**
    - proptest로 임의의 AgentRecord 생성, 직렬화 후 SSH 개인 키 내용·API 토큰·비밀번호 미포함 확인
    - **Validates: Requirements 8.1, 8.2**

- [ ] 3. 체크포인트 — 기반 타입 및 저장소 검증
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

- [ ] 4. CliDetector 구현
  - [ ] 4.1 `apps/tauri/src/fleet/cli_detector.rs` 생성 — `CliDetector`, `CliStatus` 구현
    - `detect_all()` — AWS, OCI, Azure CLI 바이너리 존재 여부 및 인증 상태 일괄 확인
    - `detect(provider)` — 개별 프로바이더 CLI 감지
    - `check_binary(binary_name)` — PATH에서 바이너리 존재 확인 (`which` 또는 `std::process::Command`)
    - `check_auth(provider)` — 인증 확인 명령 실행 (`aws sts get-caller-identity`, `oci iam user get`, `az account show`)
    - CLI 미설치 시 `installed=false`, `install_url` 제공
    - 인증 실패 시 `authenticated=false`, 재인증 안내
    - 자격증명 값 로그 미기록
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

  - [ ]* 4.2 CLI 미설치 시 비활성 상태 및 설치 안내 속성 테스트 작성
    - **Property 14: CLI 미설치 시 비활성 상태 및 설치 안내**
    - proptest로 임의의 CloudProvider에 대해, CLI 미설치 시 `installed=false`이고 `install_url`이 공식 URL 포함 확인
    - **Validates: Requirements 1.3**

- [ ] 5. SshClient 구현
  - [ ] 5.1 `apps/tauri/src/fleet/ssh_client.rs` 생성 — `SshClient` 구현
    - `execute(conn, command, timeout)` — ssh 바이너리를 `std::process::Command`로 호출, stdout/stderr/exit_code 수집
    - `check_connectivity(conn, timeout)` — SSH 포트 응답 확인
    - `validate_command(command)` — 제어 문자 필터링 (ASCII 0x00-0x1F 중 탭·개행 제외), 4096바이트 길이 제한
    - SSH 명령 생성: `-i {key_path}`, `{user}@{host}`, `-p {port}`, `StrictHostKeyChecking` 옵션
    - SSH 키 파일 미존재 시 연결 시도 안 함, 명확한 오류 반환
    - 모든 SSH 연결/명령을 AuditLogger에 기록
    - _Requirements: 3.2, 3.3, 6.1, 6.2, 6.3, 6.4, 6.7, 8.3, 8.4, 8.8_

  - [ ]* 5.2 SSH 명령 생성 정확성 속성 테스트 작성
    - **Property 6: SSH 명령 생성 정확성**
    - proptest로 임의의 SshConnectionInfo 생성, ssh 명령에 `-i`, `user@host`, `-p`, `StrictHostKeyChecking` 옵션 포함 확인
    - **Validates: Requirements 3.2, 8.3**

  - [ ]* 5.3 명령 입력 검증 속성 테스트 작성
    - **Property 12: 명령 입력 검증**
    - proptest로 임의의 문자열 생성, 제어 문자 포함 시 거부, 4096바이트 초과 시 거부, 유효 입력 통과 확인
    - **Validates: Requirements 6.7**

  - [ ]* 5.4 SSH 키 파일 미존재 시 연결 거부 속성 테스트 작성
    - **Property 15: SSH 키 파일 미존재 시 연결 거부**
    - proptest로 임의의 존재하지 않는 경로 생성, SSH 연결 시도 없이 오류 반환 확인
    - **Validates: Requirements 8.8**

- [ ] 6. Provisioner 구현
  - [ ] 6.1 `apps/tauri/src/fleet/provisioner.rs` 생성 — `Provisioner` 구현
    - `provision(spec)` — CloudProvider별 CLI 명령 실행 (`aws ec2 run-instances`, `oci compute instance launch`, `az vm create`)
    - `terminate(record)` — 인스턴스 종료 CLI 명령 실행
    - `wait_for_running(record, timeout)` — 10초 간격 폴링, 120초 타임아웃
    - `build_create_command(spec)` — InstanceSpec → CLI Command 변환
    - `parse_instance_output(provider, output)` — CLI JSON 출력에서 인스턴스 ID, 공인 IP 파싱
    - CLI 실패 시 `AgentRecord.status = Error`, stderr를 error_message에 저장
    - 보안 그룹 ID 지정 시 해당 ID 사용, 미지정 시 기본 동작
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8_

  - [ ]* 6.2 CLI 명령 생성 정확성 속성 테스트 작성
    - **Property 1: CLI 명령 생성 정확성**
    - proptest로 임의의 CloudProvider와 유효한 InstanceSpec 생성, `build_create_command`가 올바른 CLI 바이너리와 인수 포함 확인
    - **Validates: Requirements 2.1, 2.2**

  - [ ]* 6.3 인스턴스 출력 파싱 라운드트립 속성 테스트 작성
    - **Property 2: 인스턴스 출력 파싱 라운드트립**
    - proptest로 임의의 인스턴스 ID/IP 쌍 생성, 프로바이더별 JSON 생성 후 파싱하여 원래 값 복원 확인
    - **Validates: Requirements 2.3**

  - [ ]* 6.4 CLI 실패 시 Error 상태 전이 속성 테스트 작성
    - **Property 4: CLI 실패 시 Error 상태 전이**
    - proptest로 임의의 0이 아닌 종료 코드와 stderr 생성, 실패 시 status=Error, error_message에 stderr 포함 확인
    - **Validates: Requirements 2.5, 3.6**

- [ ] 7. 체크포인트 — CLI 감지 및 프로비저닝 검증
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

- [ ] 8. AgentInstaller 구현
  - [ ] 8.1 `apps/tauri/src/fleet/installer.rs` 생성 — `AgentInstaller` 구현
    - `install(record)` — SSH 접속 → 설치 스크립트 실행 → 프로세스 확인 전체 흐름
    - `wait_for_ssh(record, timeout)` — SSH 포트 응답 대기 (최대 60초)
    - `upload_and_start(record)` — naraeclaw 바이너리 복사, headless 모드 시작, 로그 경로 설정
    - `verify_process(record)` — `ps aux | grep naraeclaw`로 프로세스 실행 확인
    - 프로세스 확인 성공 시 status=Running, 실패 시 status=Error
    - 모든 SSH 명령/출력을 AuditLogger에 기록
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8_

  - [ ]* 8.2 프로세스 확인 결과에 따른 상태 전이 속성 테스트 작성
    - **Property 5: 프로세스 확인 결과에 따른 상태 전이**
    - proptest로 임의의 AgentRecord와 verify_process 결과 생성, 성공 시 Running, 실패 시 Error 확인
    - **Validates: Requirements 3.6, 3.7**

- [ ] 9. FleetPoller 구현
  - [ ] 9.1 `apps/tauri/src/fleet/poller.rs` 생성 — `FleetPoller` 구현
    - `spawn_agent_poller(app, state, agent_id)` — 에이전트별 독립 폴링 태스크 생성 (기존 `health::spawn_health_poller` 패턴)
    - 30초 간격 SSH 폴링, 10초 타임아웃
    - 연속 3회 실패 시 status=Stopped, Fleet_UI 알림 (Tauri 이벤트)
    - 중간 성공 시 consecutive_failures 카운터 리셋
    - `fetch_logs(record, lines)` — `~/.naraeclaw/agent.log` 마지막 N줄 읽기
    - `tail_logs(record)` — 로그 신규 내용 읽기
    - 각 에이전트 폴링 태스크 독립 실행, 상호 영향 없음
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8_

  - [ ]* 9.2 연속 폴링 실패 시 Stopped 상태 전이 속성 테스트 작성
    - **Property 7: 연속 폴링 실패 시 Stopped 상태 전이**
    - proptest로 임의의 폴링 결과 시퀀스 생성, 연속 3회 실패 시 Stopped, 중간 성공 시 카운터 리셋 확인
    - **Validates: Requirements 5.3, 5.7**

- [ ] 10. FleetManager 구현
  - [ ] 10.1 `apps/tauri/src/fleet/manager.rs` 생성 — `FleetManager` 구현
    - `provision_and_install(spec)` — Provisioner → AgentInstaller → FleetPoller 전체 오케스트레이션
    - `stop_agent(agent_id)` — SSH로 종료 신호 전송, status=Stopped
    - `restart_agent(agent_id)` — SSH로 재시작, 프로세스 확인 후 status=Running
    - `delete_agent(agent_id)` — CLI로 인스턴스 종료, AgentRecord 제거
    - `list_agents()` / `get_agent(agent_id)` — 목록 조회 및 상세 조회
    - `shutdown()` — 앱 종료 시 모든 폴링 태스크 정상 종료 (원격 에이전트는 유지)
    - `Arc<RwLock<FleetState>>`로 상태 관리
    - 인스턴스 종료 실패 시 AgentRecord 유지, 수동 삭제 안내
    - 모든 생명주기 작업을 AuditLogger에 기록
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8_

  - [ ]* 10.2 명령 전송 상태 게이트 속성 테스트 작성
    - **Property 11: 명령 전송 상태 게이트**
    - proptest로 임의의 AgentStatus 생성, Running일 때만 명령 허용, 그 외 거부 확인
    - **Validates: Requirements 6.5**

  - [ ]* 10.3 중지 후 AgentRecord 보존 속성 테스트 작성
    - **Property 13: 중지 후 AgentRecord 보존**
    - proptest로 임의의 AgentRecord 생성, stop_agent 후 목록에 존재하고 status만 Stopped, 나머지 메타데이터 불변 확인
    - **Validates: Requirements 7.6**

  - [ ]* 10.4 AgentRecord 필터링 정확성 속성 테스트 작성
    - **Property 8: AgentRecord 필터링 정확성**
    - proptest로 임의의 AgentRecord 목록과 필터 조건 생성, 필터 결과가 조건 만족 레코드만 포함하고 누락 없음 확인
    - **Validates: Requirements 4.8**

- [ ] 11. 체크포인트 — 핵심 백엔드 로직 검증
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

- [ ] 12. Tauri Commands 및 상태 연결
  - [ ] 12.1 `apps/tauri/src/commands/fleet.rs` 생성 — Tauri IPC 커맨드 구현
    - `detect_cloud_cli()` — CliDetector 호출
    - `create_instance(state, spec)` — FleetManager.provision_and_install 호출
    - `list_fleet_agents(state)` — FleetManager.list_agents 호출
    - `get_agent_detail(state, agent_id)` — FleetManager.get_agent 호출
    - `stop_agent(state, agent_id)` — FleetManager.stop_agent 호출
    - `restart_agent(state, agent_id)` — FleetManager.restart_agent 호출
    - `delete_agent(state, agent_id)` — FleetManager.delete_agent 호출
    - `send_remote_command(state, agent_id, command)` — SshClient.execute 호출 (Running 상태 확인 포함)
    - `fetch_agent_logs(state, agent_id, lines)` — FleetPoller.fetch_logs 호출
    - _Requirements: 4.4, 4.5, 6.1, 6.5_

  - [ ] 12.2 `apps/tauri/src/lib.rs` 수정 — FleetManager를 Tauri 상태로 등록, fleet 커맨드를 invoke_handler에 추가
    - `setup()` 클로저에서 FleetManager 초기화 및 `app.manage()` 등록
    - `invoke_handler`에 fleet 커맨드 핸들러 추가
    - 앱 종료 시 `FleetManager::shutdown()` 호출 연결 (`RunEvent::Exit`)
    - _Requirements: 7.7_

  - [ ]* 12.3 Tauri 커맨드 단위 테스트 작성
    - 각 커맨드의 정상/오류 경로 테스트
    - Running 아닌 상태에서 명령 전송 거부 확인
    - _Requirements: 6.5_

- [ ] 13. 체크포인트 — Tauri 통합 검증
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

- [ ] 14. Fleet UI React 컴포넌트 구현
  - [ ] 14.1 Fleet 목록 화면 구현 — 에이전트 목록 테이블, 상태 배지(Running=녹색, Stopped=회색, Error=빨간색, Provisioning/Installing=노란색, Terminated=어두운 회색), 필터링(CloudProvider/AgentStatus)
    - 빈 Fleet 상태 온보딩 메시지 포함
    - Tauri invoke로 `list_fleet_agents`, `detect_cloud_cli` 호출
    - _Requirements: 4.1, 4.2, 4.3, 4.7, 4.8_

  - [ ] 14.2 인스턴스 생성 폼 구현 — CloudProvider 선택, InstanceSpec 입력 (리전, 인스턴스 타입, SSH 키 경로, SSH 사용자, 보안 그룹 ID), 클라이언트 측 유효성 검증
    - CLI 미설치/인증 만료 프로바이더 비활성화 표시
    - 생성 진행 중 스피너 및 버튼 비활성화
    - _Requirements: 4.5, 4.6, 1.3, 1.4_

  - [ ] 14.3 에이전트 상세 패널 구현 — 로그 뷰어, 명령 입력창, 재시작/중지/삭제 버튼
    - 로그 뷰어: `fetch_agent_logs` 호출, 10초 간격 자동 갱신
    - 명령 입력창: `send_remote_command` 호출, 결과(stdout/stderr/exit_code) 표시
    - 삭제 확인 다이얼로그 (인스턴스 종료 및 과금 중단 경고)
    - 호스트 키 검증 비활성화 시 경고 배지 표시
    - _Requirements: 4.4, 4.6, 5.4, 5.5, 6.1, 6.2, 7.3, 8.4_

  - [ ] 14.4 Tauri 이벤트 리스너 연결 — FleetPoller의 상태 변경 이벤트를 수신하여 UI 실시간 갱신 (5초 이내)
    - _Requirements: 4.2_

- [ ] 15. 최종 체크포인트 — 전체 통합 검증
  - 모든 테스트 통과 확인, 질문이 있으면 사용자에게 문의.

## 참고

- `*` 표시된 태스크는 선택 사항이며 빠른 MVP를 위해 건너뛸 수 있음
- 각 태스크는 추적 가능성을 위해 구체적인 요구사항을 참조함
- 체크포인트는 점진적 검증을 보장함
- 속성 테스트는 `proptest` 라이브러리를 사용하여 설계 문서의 정확성 속성을 검증함
- 단위 테스트는 구체적인 예시와 엣지 케이스를 검증함
