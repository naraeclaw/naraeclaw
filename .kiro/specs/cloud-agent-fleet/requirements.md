# 요구사항 문서: Cloud Agent Fleet

## 소개

NaraeClaw 데스크탑(Tauri 2.0)에서 AWS, OCI, Azure 클라우드에 경량 naraeclaw 에이전트를 배포하고 원격으로 제어하는 기능이다. 로컬에 설치된 클라우드 CLI(`aws`, `oci`, `az`)의 인증 컨텍스트를 재사용해 인스턴스를 프로비저닝하고, SSH를 통해 에이전트를 설치·시작한 뒤 데스크탑 Fleet 화면에서 상태 모니터링 및 명령 전달을 수행한다.

핵심 동기: 클라우드 인프라(예: OCI VCN)는 외부 접속을 차단하는 경우가 많아, 해당 네트워크 내부에서 동작하는 에이전트가 필요하다. 이 기능은 별도 릴레이 인프라 없이 SSH 폴링 방식으로 원격 에이전트를 제어한다.

---

## 용어 정의

- **Fleet_Manager**: 데스크탑 내 원격 에이전트 목록과 생명주기를 관리하는 컴포넌트
- **CLI_Detector**: 로컬 클라우드 CLI 바이너리 존재 여부와 인증 상태를 확인하는 컴포넌트
- **Provisioner**: 클라우드 CLI를 호출해 원격 인스턴스를 생성·삭제하는 컴포넌트
- **SSH_Client**: Rust `std::process::Command`로 로컬 `ssh` 바이너리를 호출하는 래퍼
- **Agent_Installer**: SSH를 통해 원격 인스턴스에 naraeclaw 경량 에이전트를 설치·시작하는 컴포넌트
- **Poller**: 주기적으로 SSH를 통해 원격 에이전트 상태와 로그를 수집하는 컴포넌트
- **Remote_Agent**: 원격 인스턴스에서 headless 모드로 실행되는 naraeclaw 에이전트 프로세스
- **Fleet_UI**: 데스크탑 React 프론트엔드의 Fleet 관리 화면
- **Credential_Store**: SSH 키 경로와 클라우드 자격증명을 암호화 저장하는 컴포넌트 (기존 `naraeclaw.json` store 확장)
- **Audit_Logger**: Fleet 관련 모든 작업을 감사 로그에 기록하는 컴포넌트
- **CloudProvider**: AWS, OCI, Azure 중 하나를 나타내는 열거형 값
- **InstanceSpec**: 인스턴스 생성에 필요한 리전, 인스턴스 타입, SSH 키 경로 등의 파라미터 집합
- **AgentRecord**: Fleet에 등록된 원격 에이전트의 메타데이터 (ID, 클라우드, 리전, IP, 상태 등)
- **AgentStatus**: 에이전트의 현재 상태 — `Provisioning`, `Installing`, `Running`, `Stopped`, `Error`, `Terminated`

---

## 요구사항

### 요구사항 1: 클라우드 CLI 인증 감지

**사용자 스토리:** 데스크탑 사용자로서, 로컬에 설치된 클라우드 CLI의 인증 상태를 확인하고 싶다. 그래야 별도 자격증명 입력 없이 기존 CLI 세션을 재사용해 인스턴스를 생성할 수 있다.

#### 인수 기준

1. WHEN 사용자가 Fleet 화면을 열면, THE CLI_Detector SHALL 로컬 `PATH`에서 `aws`, `oci`, `az` 바이너리의 존재 여부를 각각 확인한다.
2. WHEN CLI 바이너리가 존재하면, THE CLI_Detector SHALL 해당 CLI의 인증 상태 확인 명령(`aws sts get-caller-identity`, `oci iam user get --user-id $(oci iam user list --query 'data[0].id' --raw-output)`, `az account show`)을 실행해 인증 유효성을 검증한다.
3. IF CLI 바이너리가 존재하지 않으면, THEN THE CLI_Detector SHALL 해당 클라우드 프로바이더를 비활성 상태로 표시하고 설치 안내 링크를 제공한다.
4. IF 인증 확인 명령이 0이 아닌 종료 코드를 반환하면, THEN THE CLI_Detector SHALL 해당 프로바이더를 인증 만료 상태로 표시하고 재인증 방법을 안내한다.
5. WHEN CLI 인증 상태 확인이 완료되면, THE CLI_Detector SHALL 각 프로바이더별 상태(설치됨/인증됨/미설치/인증만료)를 Fleet_UI에 전달한다.
6. THE CLI_Detector SHALL 인증 확인 명령 실행 시 표준 출력과 표준 오류를 캡처하되, 자격증명 값을 로그에 기록하지 않는다.

---

### 요구사항 2: 인스턴스 프로비저닝

**사용자 스토리:** 데스크탑 사용자로서, 원하는 클라우드에 에이전트용 인스턴스를 생성하고 싶다. 그래야 해당 클라우드 네트워크 내부에서 동작하는 에이전트를 확보할 수 있다.

#### 인수 기준

1. WHEN 사용자가 인스턴스 생성 폼을 제출하면, THE Provisioner SHALL 선택된 CloudProvider에 따라 해당 CLI 명령을 실행해 인스턴스를 생성한다.
   - AWS: `aws ec2 run-instances`
   - OCI: `oci compute instance launch`
   - Azure: `az vm create`
2. THE Provisioner SHALL InstanceSpec에 포함된 리전, 인스턴스 타입, SSH 공개 키, 보안 그룹(또는 동등한 네트워크 정책)을 CLI 명령 인수로 전달한다.
3. WHEN 인스턴스 생성 CLI 명령이 실행되면, THE Provisioner SHALL 명령 출력에서 인스턴스 ID와 공인 IP 주소(또는 DNS 이름)를 파싱해 AgentRecord에 저장한다.
4. WHILE 인스턴스가 `Provisioning` 상태이면, THE Provisioner SHALL 10초 간격으로 인스턴스 상태를 폴링하고 `running` 상태가 될 때까지 대기한다.
5. IF 인스턴스 생성 CLI 명령이 0이 아닌 종료 코드를 반환하면, THEN THE Provisioner SHALL AgentRecord 상태를 `Error`로 설정하고 CLI 표준 오류 내용을 오류 메시지로 저장한다.
6. IF 인스턴스가 120초 이내에 `running` 상태에 도달하지 않으면, THEN THE Provisioner SHALL 타임아웃 오류를 기록하고 AgentRecord 상태를 `Error`로 설정한다.
7. THE Provisioner SHALL 인스턴스 생성 전 InstanceSpec의 필수 필드(리전, 인스턴스 타입, SSH 키 경로)가 모두 채워졌는지 검증한다.
8. WHERE 사용자가 기존 보안 그룹 ID를 지정한 경우, THE Provisioner SHALL 새 보안 그룹을 생성하지 않고 지정된 ID를 사용한다.

---

### 요구사항 3: 에이전트 설치

**사용자 스토리:** 데스크탑 사용자로서, 새로 생성된 인스턴스에 naraeclaw 에이전트가 자동으로 설치되고 시작되길 원한다. 그래야 수동 SSH 접속 없이 바로 원격 에이전트를 사용할 수 있다.

#### 인수 기준

1. WHEN 인스턴스가 `running` 상태에 도달하면, THE Agent_Installer SHALL SSH를 통해 원격 인스턴스에 접속해 naraeclaw 설치 스크립트를 실행한다.
2. THE Agent_Installer SHALL SSH 접속 시 AgentRecord에 저장된 SSH 키 경로와 사용자명을 사용하며, 호스트 키 검증을 수행한다.
3. THE Agent_Installer SHALL 설치 스크립트 실행 전 원격 인스턴스의 SSH 포트(기본 22)가 응답할 때까지 최대 60초 대기한다.
4. THE Agent_Installer SHALL 설치 스크립트를 통해 naraeclaw 바이너리를 원격 인스턴스에 복사하고, headless 모드로 시작하며, 로그 파일 경로(`~/.naraeclaw/agent.log`)를 설정한다.
5. WHEN 설치 스크립트가 완료되면, THE Agent_Installer SHALL `ps aux | grep naraeclaw` 또는 동등한 명령으로 에이전트 프로세스 실행 여부를 확인한다.
6. IF 에이전트 프로세스 확인에 실패하면, THEN THE Agent_Installer SHALL AgentRecord 상태를 `Error`로 설정하고 설치 로그를 오류 메시지에 포함한다.
7. WHEN 에이전트 프로세스 실행이 확인되면, THE Agent_Installer SHALL AgentRecord 상태를 `Running`으로 업데이트하고 Fleet_Manager에 등록 완료를 알린다.
8. THE Agent_Installer SHALL 설치 과정의 모든 SSH 명령과 출력을 Audit_Logger에 기록한다.

---

### 요구사항 4: Fleet 관리 UI

**사용자 스토리:** 데스크탑 사용자로서, 모든 원격 에이전트를 한 화면에서 확인하고 관리하고 싶다. 그래야 여러 클라우드에 분산된 에이전트를 효율적으로 운영할 수 있다.

#### 인수 기준

1. THE Fleet_UI SHALL Fleet 화면에서 등록된 모든 AgentRecord를 목록으로 표시하며, 각 항목에 에이전트 ID, CloudProvider, 리전, 공인 IP, AgentStatus, 마지막 확인 시각을 포함한다.
2. WHEN AgentStatus가 변경되면, THE Fleet_UI SHALL 해당 AgentRecord 항목의 상태 표시를 5초 이내에 갱신한다.
3. THE Fleet_UI SHALL AgentStatus별로 시각적으로 구분된 상태 배지를 표시한다 (`Running`=녹색, `Stopped`=회색, `Error`=빨간색, `Provisioning`/`Installing`=노란색, `Terminated`=어두운 회색).
4. WHEN 사용자가 AgentRecord 항목을 선택하면, THE Fleet_UI SHALL 해당 에이전트의 상세 패널을 표시하며 로그 뷰어, 명령 입력창, 재시작/중지/삭제 버튼을 포함한다.
5. THE Fleet_UI SHALL 인스턴스 생성 버튼과 CloudProvider 선택, InstanceSpec 입력 폼을 제공한다.
6. WHEN 인스턴스 생성 또는 삭제 작업이 진행 중이면, THE Fleet_UI SHALL 해당 AgentRecord 항목에 진행 중 스피너를 표시하고 중복 작업 버튼을 비활성화한다.
7. THE Fleet_UI SHALL 빈 Fleet 상태에서 첫 에이전트 생성을 안내하는 온보딩 메시지를 표시한다.
8. WHERE 사용자가 필터를 적용한 경우, THE Fleet_UI SHALL CloudProvider 또는 AgentStatus 기준으로 AgentRecord 목록을 필터링해 표시한다.

---

### 요구사항 5: SSH 폴링 모니터링

**사용자 스토리:** 데스크탑 사용자로서, 원격 에이전트의 상태와 로그를 실시간에 가깝게 확인하고 싶다. 그래야 에이전트 이상 상황을 빠르게 감지할 수 있다.

#### 인수 기준

1. WHILE AgentRecord의 상태가 `Running`이면, THE Poller SHALL 30초 간격으로 SSH를 통해 원격 에이전트 프로세스 생존 여부를 확인한다.
2. WHEN 폴링 주기가 도래하면, THE Poller SHALL `ps aux | grep naraeclaw` 명령으로 프로세스 존재 여부를 확인하고 결과를 AgentRecord에 반영한다.
3. IF 연속 3회 폴링에서 프로세스가 감지되지 않으면, THEN THE Poller SHALL AgentRecord 상태를 `Stopped`로 변경하고 Fleet_UI에 알림을 전송한다.
4. WHEN 사용자가 에이전트 상세 패널의 로그 뷰어를 열면, THE Poller SHALL SSH를 통해 `~/.naraeclaw/agent.log`의 마지막 200줄을 읽어 표시한다.
5. WHILE 로그 뷰어가 열려 있으면, THE Poller SHALL 10초 간격으로 로그 파일의 신규 내용을 읽어 뷰어에 추가한다.
6. THE Poller SHALL 각 SSH 폴링 연결에 10초 타임아웃을 적용하며, 타임아웃 발생 시 해당 폴링 결과를 실패로 처리한다.
7. IF SSH 연결이 타임아웃되거나 거부되면, THEN THE Poller SHALL 해당 폴링 실패를 기록하되 즉시 AgentStatus를 변경하지 않고 연속 실패 횟수를 누적한다.
8. THE Poller SHALL 동시에 실행 중인 모든 AgentRecord에 대해 독립적인 폴링 태스크를 유지하며, 한 에이전트의 폴링 지연이 다른 에이전트에 영향을 주지 않는다.

---

### 요구사항 6: 원격 명령 전달

**사용자 스토리:** 데스크탑 사용자로서, 원격 에이전트에 명령을 전달하고 결과를 확인하고 싶다. 그래야 별도 SSH 클라이언트 없이 데스크탑에서 원격 에이전트를 직접 제어할 수 있다.

#### 인수 기준

1. WHEN 사용자가 명령 입력창에 명령을 입력하고 전송하면, THE SSH_Client SHALL 해당 AgentRecord의 SSH 접속 정보를 사용해 원격 인스턴스에 명령을 실행한다.
2. THE SSH_Client SHALL 명령 실행 결과(표준 출력, 표준 오류, 종료 코드)를 수집해 Fleet_UI의 명령 결과 영역에 표시한다.
3. THE SSH_Client SHALL 단일 명령 실행에 60초 타임아웃을 적용하며, 타임아웃 발생 시 오류 메시지를 반환한다.
4. IF 명령 실행 중 SSH 연결이 끊어지면, THEN THE SSH_Client SHALL 오류 메시지와 함께 부분 출력(있는 경우)을 Fleet_UI에 반환한다.
5. THE SSH_Client SHALL 명령 전달 전 AgentRecord 상태가 `Running`인지 확인하며, `Running`이 아닌 경우 명령 전송을 거부하고 사용자에게 상태를 안내한다.
6. THE Audit_Logger SHALL 전달된 모든 명령, 실행 시각, 종료 코드를 감사 로그에 기록한다.
7. THE SSH_Client SHALL 명령 입력창에서 허용되지 않는 제어 문자를 필터링하고 명령 길이를 4096바이트로 제한한다.

---

### 요구사항 7: 인스턴스 생명주기 관리

**사용자 스토리:** 데스크탑 사용자로서, 원격 에이전트 인스턴스를 시작, 중지, 재시작, 삭제할 수 있길 원한다. 그래야 불필요한 클라우드 비용을 줄이고 에이전트를 유연하게 운영할 수 있다.

#### 인수 기준

1. WHEN 사용자가 `중지` 버튼을 클릭하면, THE Fleet_Manager SHALL SSH를 통해 원격 에이전트 프로세스에 종료 신호를 전송하고 AgentRecord 상태를 `Stopped`로 업데이트한다.
2. WHEN 사용자가 `재시작` 버튼을 클릭하면, THE Fleet_Manager SHALL SSH를 통해 원격 인스턴스에서 naraeclaw 에이전트를 재시작하고 프로세스 실행을 확인한 뒤 AgentRecord 상태를 `Running`으로 업데이트한다.
3. WHEN 사용자가 `삭제` 버튼을 클릭하면, THE Fleet_UI SHALL 삭제 확인 다이얼로그를 표시하며 인스턴스 종료 및 과금 중단 경고를 포함한다.
4. WHEN 사용자가 삭제를 확인하면, THE Fleet_Manager SHALL 해당 CloudProvider의 CLI를 호출해 인스턴스를 종료하고 AgentRecord를 Fleet에서 제거한다.
5. IF 인스턴스 종료 CLI 명령이 실패하면, THEN THE Fleet_Manager SHALL 오류를 기록하고 AgentRecord를 Fleet에 유지하며 사용자에게 수동 삭제 방법을 안내한다.
6. THE Fleet_Manager SHALL 인스턴스 중지 후에도 AgentRecord(메타데이터)를 Fleet에 유지하며, 사용자가 명시적으로 삭제하기 전까지 이력을 보존한다.
7. WHEN 데스크탑 앱이 종료되면, THE Fleet_Manager SHALL 실행 중인 모든 폴링 태스크를 정상 종료하되 원격 에이전트 프로세스는 계속 실행 상태로 유지한다.
8. THE Audit_Logger SHALL 모든 생명주기 작업(생성, 중지, 재시작, 삭제)의 시각, 대상 AgentRecord ID, 결과를 감사 로그에 기록한다.

---

### 요구사항 8: 보안 및 자격증명 관리

**사용자 스토리:** 데스크탑 사용자로서, SSH 키와 클라우드 자격증명이 안전하게 관리되길 원한다. 그래야 민감한 정보가 평문으로 저장되거나 유출되지 않는다.

#### 인수 기준

1. THE Credential_Store SHALL SSH 키 파일 경로를 저장할 때 파일 시스템 경로만 저장하며, SSH 개인 키 내용을 메모리나 디스크에 복사하지 않는다.
2. THE Credential_Store SHALL AgentRecord를 디스크에 저장할 때 공인 IP, 인스턴스 ID, SSH 키 경로를 포함하되, 클라우드 API 토큰이나 비밀번호를 평문으로 저장하지 않는다.
3. THE SSH_Client SHALL SSH 연결 시 `StrictHostKeyChecking=yes` 옵션을 기본으로 사용하며, 최초 연결 시 호스트 키를 사용자에게 확인 요청한다.
4. WHERE 사용자가 호스트 키 검증을 명시적으로 비활성화한 경우, THE SSH_Client SHALL 해당 설정을 Audit_Logger에 기록하고 Fleet_UI에 경고 배지를 표시한다.
5. THE Audit_Logger SHALL 모든 SSH 연결 시도(성공/실패), 명령 실행, 생명주기 작업을 타임스탬프와 함께 `~/.naraeclaw/fleet-audit.log`에 기록한다.
6. THE Audit_Logger SHALL 감사 로그에 SSH 개인 키 내용, 클라우드 API 응답의 자격증명 필드를 기록하지 않는다.
7. THE CLI_Detector SHALL 클라우드 CLI 인증 확인 시 환경 변수(`AWS_ACCESS_KEY_ID`, `OCI_CLI_KEY_FILE` 등)를 로그에 출력하지 않는다.
8. IF SSH 키 파일이 존재하지 않거나 읽기 권한이 없으면, THEN THE SSH_Client SHALL 연결을 시도하지 않고 명확한 오류 메시지를 Fleet_UI에 표시한다.
9. THE Fleet_Manager SHALL AgentRecord 데이터를 기존 `naraeclaw.json` Tauri store에 저장하며, store 파일의 파일 시스템 권한이 소유자 읽기/쓰기(0600)로 설정되어 있는지 확인한다.
