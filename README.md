# 나래클로 (NaraeClaw)

> 나래 — 날개의 고어. 가볍게, 빠르게.

NaraeClaw는 한국어 우선의 경량 자율 에이전트입니다.
서버/인프라 관리, 반복 운영 작업, 개인 지식 워크플로우를 CLI와 게이트웨이 API로 다룹니다.

Desktop(Tauri) 앱과 독립 Web UI는 2026-05-05에 제품 범위에서 제거되었습니다.
현재 지원하는 제품 표면은 CLI와 게이트웨이 API입니다.

이 저장소는 ZeroClaw에서 갈라져 나온 포크이며, 현재는 한국어 환경과 실사용 범위에 맞춰 단순하게 유지하는 쪽에 맞춰져 있습니다.

## What it is

- 한국어 우선의 에이전트 런타임
- 서버 상태 확인, 운영 작업 자동화, ByoriDB 기반 영속 지식·관계 그래프 관리
- CLI와 게이트웨이 API로 접근

## Who it is for

- 개인 서버나 소규모 인프라를 직접 관리하는 사용자
- 개인 지식, 운영 메모, 반복 작업을 한곳에서 다루고 싶은 사용자
- 빠르게 설치해서 바로 쓰는 로컬 중심 워크플로우를 원하는 사용자

## Core surfaces

- `CLI` - 온보딩, 에이전트 실행, 상태 확인, 설정 관리
- `Gateway` - HTTP/WebSocket API (포트 42617)

## Quick start

```bash
# 영속 지식 저장소 설치 (macOS / Linux)
curl -fsSL https://github.com/byoridb/byori/releases/latest/download/install.sh | bash

# 빌드
cargo build --release

# 초기 설정
./target/release/naraeclaw onboard

# 에이전트 실행 (대화형 CLI)
./target/release/naraeclaw agent

# 게이트웨이 서비스 시작 (HTTP/WebSocket API)
./target/release/naraeclaw gateway
```

ByoriDB는 워크스페이스별로 격리된 기본·유일 영속 지식 저장소입니다. 여기서
“유일”은 NaraeClaw 런타임이 레거시 메모리 backend와 동시에 쓰지 않는다는 뜻이며,
중요 데이터의 별도 백업까지 대체한다는 뜻은 아닙니다. 설치·설정·기존 데이터 이전
방법은 [ByoriDB 지식 설정 가이드](docs/setup-guides/byoridb-knowledge.md)를 참고하세요.

## Current scope

- 한국어 UX와 기본 설정 흐름
- 서버/인프라 관리용 에이전트 실행
- 개인 지식 워크플로우와 ByoriDB 지식 그래프
- 파일, 셸, HTTP, 브라우저, ByoriDB 지식 같은 핵심 도구
- 게이트웨이 API를 통한 외부 연동

## Validation basics

변경이 있으면 보통 아래만 확인하면 충분합니다.

```bash
cargo fmt --all -- --check
cargo check --workspace
# 변경 범위에 맞는 targeted clippy/test를 추가로 실행
```

현재 GitHub Fast CI도 format과 workspace check를 필수 경로로 사용합니다. 전체
Clippy·테스트·로컬 Docker 검증은 변경 위험도에 맞춰 추가합니다.

## License and provenance

NaraeClaw는 [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)에서 출발한 포크입니다.

- 라이선스: `MIT OR Apache-2.0`
- 원본 프로젝트: ZeroClaw, Copyright 2025 ZeroClaw Labs
- 포크 및 변경분: NaraeClaw contributors
- 자세한 고지: [NOTICE](NOTICE), [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)

NaraeClaw는 공식 Zeroclaw 프로젝트가 아니며, upstream 프로젝트와의 제휴나 보증을 의미하지 않습니다.
