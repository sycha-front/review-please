# review-please

macOS 시스템 트레이에서 Slack 멘션 기반 PR 리뷰 요청을 추적하는 Tauri 앱이다.

GitHub Release에 올린 signed updater artifact를 앱이 직접 내려받는 반자동 업데이트 방식을 쓴다.

## 운영 방식

- 최초 설치는 GitHub Release에 올린 `.dmg`로 한다.
- 앱 안에서 새 버전이 보이면 `원클릭 업데이트`로 최신 릴리즈를 설치한다.
- 업데이트 검증은 Tauri updater 서명키를 사용한다.
- 앱을 다시 빌드할 때는 항상 같은 signing key pair를 계속 써야 한다.
- 앱 첫 실행 시 설정의 `로컬 repo 경로`는 현재 빌드에 사용된 저장소 경로를 기본값으로 채운다.

## 사전 조건

- macOS
- Node.js + yarn
- Rust toolchain
- Xcode Command Line Tools
- GitHub Release에 파일을 업로드할 권한
- Tauri updater signing private key
- 위 private key에 대응하는 public key

## 최초 설치

개발자가 직접 로컬에서 설치해서 테스트할 때는 아래 명령을 쓴다.

```bash
yarn app:install
```

설치 위치:

```text
~/Applications/review-please.app
```

배포받는 사용자는 GitHub Release에서 `.dmg`를 받아 설치하면 된다.

## 업데이트

앱 안에서도 새 버전이 감지되면 헤더 아래 배너에서 `원클릭 업데이트`를 눌러 최신 릴리즈 아티팩트를 다시 설치할 수 있다.

내부적으로 아래를 수행한다.

- `latest.json` 조회
- 서명된 `.app.tar.gz` 다운로드
- signature 검증
- 앱 교체 후 재시작

## 배포

아래 절차대로 하면 GitHub Release 기반 배포가 된다.

### 1. 버전 올리기

아래 두 파일의 버전을 같은 값으로 올린다.

- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

예시:

```toml
version = "0.1.1"
```

```json
"version": "0.1.1"
```

### 2. 배포 환경 파일 만들기

예시 파일을 복사해서 `.env.release`를 만든다.

```bash
cp .env.release.example .env.release
```

`.env.release`에서 최소한 아래 값을 채운다.

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- `TAURI_UPDATER_PUBLIC_KEY_FILE` 또는 `TAURI_UPDATER_PUBLIC_KEY`
- `GITHUB_REPOSITORY`

기본 예시는 현재 저장소 기준으로 이미 들어 있다.

### 3. 릴리즈 파일 만들기

```bash
yarn app:release
```

이 명령은 아래를 자동으로 수행한다.

- `.env.release` 로드
- updater public key를 앱에 bake-in
- signed updater artifact 생성
- `release/v버전/` 폴더 생성
- `latest.json` 생성

예를 들어 `0.1.1` 버전이면 아래 폴더가 생긴다.

```text
release/v0.1.1/
```

그 안에는 보통 아래 파일이 들어 있다.

- `latest.json`
- `review-please.app.tar.gz`
- `review-please.app.tar.gz.sig`
- `*.dmg`

### 4. GitHub Release 올리기

GitHub에서 tag를 `v버전` 형식으로 만든다.

예:

```text
v0.1.1
```

그 Release에 아래 파일을 업로드한다.

- `latest.json`
- `review-please.app.tar.gz`
- `review-please.app.tar.gz.sig`
- `*.dmg`

중요:

- `latest.json` 안의 URL은 같은 tag의 asset 이름을 가리킨다.
- tag 이름이 `v버전` 형식과 다르면 앱 업데이트가 깨진다.
- Draft나 pre-release가 아니라 일반 Release로 publish 해야 한다.
- 이후 버전도 반드시 같은 signing key pair로 빌드해야 기존 설치본이 업데이트를 신뢰한다.

### 5. 사용자 설치와 업데이트

- 신규 사용자: Release의 `.dmg` 설치
- 기존 사용자: 앱에서 `원클릭 업데이트`

## 로그인 자동 실행 해제

```bash
yarn app:login-disable
```

평소에는 앱 설정의 토글을 쓰는 쪽을 권장한다.

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
yarn app:release
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

## 참고

- 이 README의 배포 절차는 빌드한 Mac과 같은 CPU 아키텍처 사용자 기준이다.
- Intel Mac과 Apple Silicon 둘 다 배포하려면 각각 별도 빌드와 별도 `latest.json` 병합이 필요하다.
- macOS 보안 경고가 뜨면 첫 실행 시 Finder에서 앱 우클릭 후 `열기`로 허용하면 된다.
