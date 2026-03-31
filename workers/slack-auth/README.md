# Slack Auth Worker

Slack OAuth callback/code exchange 전용 Cloudflare Worker입니다.

## Required Secrets

- `SLACK_CLIENT_ID`
- `SLACK_CLIENT_SECRET`
- `SLACK_REDIRECT_URI`
- `AUTH_SESSIONS` KV binding

## Endpoints

- `POST /slack/auth/session`
- `GET /slack/auth/session/:sessionId`
- `GET /slack/oauth/callback`

앱은 배포된 production Worker URL을 코드에 내장해서 사용합니다.
