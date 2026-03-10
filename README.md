# pr-please

macOS 시스템 트레이에서 Slack 멘션 기반 PR 리뷰 요청을 추적하는 Tauri 앱이다.

## 실행 방법

사전 조건:

- macOS
- `yarn`
- Rust toolchain
- 시스템 기본 명령 사용 가능
  - `curl`
  - `sqlite3`
  - `security`
  - `osascript`

개발 서버 실행:

```bash
yarn install
yarn tauri dev
```

프로덕션 빌드:

```bash
yarn tauri build
```

CLI 설정 예시:

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- setup \
  --keyword "@review-me" \
  --slack-token xoxp-... \
  --github-token ghp_...
```

상태 확인:

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- doctor
cargo run --manifest-path src-tauri/Cargo.toml -- dump --format json
cargo run --manifest-path src-tauri/Cargo.toml -- sync-once
```

로컬 상태 초기화:

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- reset-state
cargo run --manifest-path src-tauri/Cargo.toml -- clear-credentials
```

## 동작 방식

- 앱 시작 시 Dock 아이콘은 숨기고 시스템 트레이에 상주한다.
- 트레이 아이콘을 왼쪽 클릭하면 아이콘 아래쪽에 팝업 윈도우가 뜬다.
- 팝업 윈도우는 현재 흰 배경 기본 화면이며, 내부에 `Main` 컨테이너 하나만 둔 상태다.
- 트레이 메뉴는 오른쪽 클릭으로 열 수 있다.
- Slack/GitHub 동기화는 Rust 백엔드에서 수행한다.

## 폴더 구조

```text
.
├── src
│   ├── App.tsx
│   └── main.tsx
├── src-tauri
│   ├── src
│   │   ├── app.rs
│   │   ├── cli.rs
│   │   ├── config.rs
│   │   ├── db.rs
│   │   ├── keychain.rs
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   ├── models.rs
│   │   ├── providers
│   │   │   ├── github.rs
│   │   │   ├── mod.rs
│   │   │   └── slack.rs
│   │   ├── services
│   │   │   ├── github_events.rs
│   │   │   ├── mod.rs
│   │   │   ├── notification.rs
│   │   │   ├── review_state.rs
│   │   │   ├── slack_ingest.rs
│   │   │   └── sync.rs
│   │   └── tray.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
└── README.md
```

## 현재 UI 상태

- `src/App.tsx`: 팝업 윈도우의 기본 흰 배경 레이아웃
- `Main` 텍스트가 들어간 단일 컨테이너만 생성해 둔 상태
- 실제 리뷰 목록 UI는 이후 여기에 추가하면 된다
