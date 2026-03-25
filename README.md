# review-please

macOS 시스템 트레이에서 Slack 멘션 기반 PR 리뷰 요청을 추적하는 Tauri 앱이다.

자동업데이트 대신, 개발자 팀이 쓰기 쉬운 반자동 배포 방식으로 운영한다.

## 운영 방식

- 앱은 한 번 빌드해서 `~/Applications/review-please.app` 에 설치한다.
- 평소 실행은 앱 번들을 직접 열어서 한다.
- 로그인 시 자동 실행은 앱 설정에서 `LaunchAgent`로 켜고 끈다.
- 업데이트는 repo에서 최신 코드를 당긴 뒤 스크립트로 다시 빌드/설치한다.
- 앱 첫 실행 시 설정의 `로컬 repo 경로`는 현재 빌드에 사용된 저장소 경로를 기본값으로 채운다.

즉, `yarn tauri dev`는 개발할 때만 쓰고, 평소 사용은 설치된 앱으로 한다.

## 사전 조건

- macOS
- Node.js + yarn
- Rust toolchain
- Xcode Command Line Tools
- 시스템 기본 명령 사용 가능
  - `curl`
  - `security`
  - `sqlite3`
  - `osascript`
  - `launchctl`

## 최초 설치

1. 저장소 clone

```bash
git clone git@github.com:sycha-front/pr-review-please.git
cd pr-review-please
```

2. 앱 빌드 + 설치

```bash
yarn app:install
```

설치가 끝나면 아래 위치에 앱이 생긴다.

```text
~/Applications/review-please.app
```

3. 앱 실행

```bash
open ~/Applications/review-please.app
```

앱 안의 설정에서 `로그인 시 자동 실행`을 켜면 다음 로그인부터 자동으로 실행된다.

## 업데이트

최신 코드를 받아서 다시 빌드/설치하고 앱을 다시 띄운다.

```bash
yarn app:update
```

앱 안에서도 새 버전이 감지되면 헤더 아래 배너에서 `원클릭 업데이트`를 눌러 같은 작업을 시작할 수 있다.
이 기능은 설정의 `로컬 repo 경로`를 사용한다.

내부적으로 아래를 수행한다.

- `git pull --ff-only`
- `yarn install --frozen-lockfile`
- 앱 재빌드
- `~/Applications/review-please.app` 덮어쓰기
- 실행 중인 앱 종료 후 재실행

## 로그인 자동 실행 해제

```bash
yarn app:login-disable
```

스크립트 방식도 남아 있지만, 평소에는 앱 설정의 토글을 쓰는 쪽을 권장한다.

## 앱 제거

```bash
yarn app:uninstall
```

이 명령은 아래를 수행한다.

- 로그인 자동 실행 해제
- 실행 중인 앱 종료
- `~/Applications/review-please.app` 삭제

## 개발 모드

UI나 Rust 로직을 개발할 때만 dev 모드를 쓴다.

```bash
yarn install
yarn tauri dev
```

## 자주 쓰는 명령

```bash
yarn app:build
yarn app:install
yarn app:update
yarn app:login-enable
yarn app:login-disable
yarn app:uninstall
```

## 설정/데이터 위치

- 설정 파일
  - `~/Library/Application Support/review-please/config.toml`
- 로컬 DB
  - `~/Library/Application Support/review-please/state.sqlite3`
- 앱 토큰
  - macOS Keychain 저장

기존 `pr-please` 경로를 쓰고 있었다면 앱 시작 시 자동으로 `review-please` 경로로 옮긴다.

## 폴더 구조

```text
.
├── scripts
│   ├── build-app.sh
│   ├── install-app.sh
│   ├── update-app.sh
│   ├── enable-login.sh
│   ├── disable-login.sh
│   └── uninstall-app.sh
├── src
│   ├── App.tsx
│   ├── features
│   ├── hooks
│   └── main.tsx
├── src-tauri
│   ├── src
│   │   ├── app.rs
│   │   ├── cli.rs
│   │   ├── commands.rs
│   │   ├── config.rs
│   │   ├── db.rs
│   │   ├── keychain.rs
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   ├── models.rs
│   │   ├── providers
│   │   ├── services
│   │   └── tray.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
└── README.md
```

## 참고

- 자동업데이트는 현재 사용하지 않는다.
- 소수 개발자 배포를 전제로, 설치형 앱 + 수동 업데이트 스크립트 방식으로 운영한다.
- macOS 보안 경고가 뜨면 첫 실행 시 Finder에서 앱 우클릭 후 `열기`로 허용하면 된다.
