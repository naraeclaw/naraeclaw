# 나래클로 (NaraeClaw)

> 나래 — 날개의 고어. 가볍게, 빠르게.

NaraeClaw는 한국어 우선의 경량 자율 에이전트입니다.
서버/인프라 관리, 반복 운영 작업, 개인 지식 워크플로우를 CLI, Desktop, Web에서 다룹니다.

이 저장소는 ZeroClaw에서 갈라져 나온 포크이며, 현재는 한국어 환경과 실사용 범위에 맞춰 단순하게 유지하는 쪽에 맞춰져 있습니다.

## What it is

- 한국어 우선의 에이전트 런타임
- 서버 상태 확인, 운영 작업 자동화, 장기 메모리 기반 개인 지식 관리
- CLI, Desktop, Web을 같은 런타임 위에서 연결

## Who it is for

- 개인 서버나 소규모 인프라를 직접 관리하는 사용자
- 개인 지식, 운영 메모, 반복 작업을 한곳에서 다루고 싶은 사용자
- 빠르게 설치해서 바로 쓰는 로컬 중심 워크플로우를 원하는 사용자

## Core surfaces

- `CLI` - 온보딩, 에이전트 실행, 상태 확인, 설정 관리
- `Desktop` - 경량 컴패니언 앱
- `Web` - 게이트웨이 기반 브라우저 연결

## Quick start

```bash
# 빌드
cargo build --release

# 초기 설정
./target/release/naraeclaw onboard

# 에이전트 실행
./target/release/naraeclaw agent

# 웹 게이트웨이
./target/release/naraeclaw gateway start

# 데스크톱 컴패니언 앱 연결
./target/release/naraeclaw desktop
```

## Current scope

- 한국어 UX와 기본 설정 흐름
- 서버/인프라 관리용 에이전트 실행
- 개인 지식 워크플로우와 장기 메모리
- 파일, 셸, HTTP, 브라우저, 메모리 같은 핵심 도구
- Desktop 및 Web 연결

## Validation basics

변경이 있으면 보통 아래만 확인하면 충분합니다.

```bash
cargo fmt --all -- --check
cargo check --workspace --exclude naraeclaw-desktop
cargo clippy --workspace --exclude naraeclaw-desktop --all-targets -- -D warnings
# 변경 범위에 맞는 targeted test를 추가로 실행
```

## License and provenance

NaraeClaw는 [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)에서 출발한 포크입니다.

- 라이선스: `MIT OR Apache-2.0`
- 원본 프로젝트: ZeroClaw, Copyright 2025 ZeroClaw Labs
- 포크 및 변경분: NaraeClaw contributors
- 자세한 고지: [NOTICE](NOTICE), [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)

NaraeClaw는 공식 Zeroclaw 프로젝트가 아니며, upstream 프로젝트와의 제휴나 보증을 의미하지 않습니다.
