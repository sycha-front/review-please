# review-please

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

로컬 릴리스 빌드에서 updater까지 포함하려면:

```bash
export TAURI_UPDATER_PUBKEY="여기에 updater public key"
export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/review-please.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
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
- 앱 시작 시 GitHub Releases 기반 업데이트를 자동 확인한다.
- 트레이 메뉴의 `Check for Updates`로 수동 확인도 할 수 있다.

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

## GitHub Releases 자동업데이트

현재 updater endpoint 기본값은 아래 GitHub Releases JSON을 바라본다.

- `https://github.com/sycha-front/pr-review-please/releases/latest/download/latest.json`

앱은 build 시점의 환경변수로 updater 공개키를 읽는다.

- `TAURI_UPDATER_PUBKEY`
- 선택: `PR_PLEASE_UPDATER_ENDPOINT`

중요:

- updater 서명 키는 `.env`가 아니라 빌드 환경변수로 넣어야 한다.
- updater private key는 절대 저장소에 커밋하면 안 된다.
- `src-tauri/tauri.conf.json`에는 `createUpdaterArtifacts: true`가 켜져 있어서 릴리스 시 `latest.json`, `.sig`, macOS updater bundle 생성이 가능하다.

### 1. updater 키 1회 생성

Node가 설치된 환경에서 Tauri CLI로 1회 생성한다.

```bash
yarn tauri signer generate -w ~/.tauri/review-please.key
```

여기서 나온 값 중:

- private key: `TAURI_SIGNING_PRIVATE_KEY` 로 GitHub Actions secret에 저장
- public key: `TAURI_UPDATER_PUBKEY` 로 GitHub Actions secret에 저장

### 2. GitHub Actions secret 준비

릴리스 워크플로우 `.github/workflows/release.yml` 에 맞춰 아래 secret이 필요하다.

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- `TAURI_UPDATER_PUBKEY`
- `APPLE_ID`
- `APPLE_PASSWORD`
- `APPLE_TEAM_ID`
- `APPLE_SIGNING_IDENTITY`
- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `KEYCHAIN_PASSWORD`

### 3. 배포 방법

버전을 올리고 태그를 푸시하면 GitHub Actions가 macOS 릴리스를 만든다.

```bash
git tag v0.1.1
git push origin v0.1.1
```

워크플로우가 성공하면:

- GitHub Release가 생성됨
- macOS updater bundle과 signature가 업로드됨
- `latest.json`이 릴리스 asset으로 올라감

배포 대상 사용자는 설치 후 앱을 실행하면 자동으로 업데이트를 확인하고, 트레이 메뉴에서도 수동 확인할 수 있다.
