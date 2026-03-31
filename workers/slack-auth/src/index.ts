type Env = {
  AUTH_SESSIONS: KVNamespace;
  SLACK_CLIENT_ID: string;
  SLACK_CLIENT_SECRET: string;
  SLACK_REDIRECT_URI: string;
};

type StoredSession =
  | {
      status: "pending";
      sessionId: string;
      sessionSecret: string;
      expiresAt: string;
    }
  | {
      status: "completed";
      sessionId: string;
      sessionSecret: string;
      expiresAt: string;
      accessToken: string;
      slackUserId: string;
      slackDisplayName: string;
      teamId: string;
      teamName: string;
      scope: string;
    }
  | {
      status: "failed";
      sessionId: string;
      sessionSecret: string;
      expiresAt: string;
      error: string;
    };

const USER_SCOPE = "search:read,users:read";
const SESSION_TTL_SECONDS = 600;

function json(data: unknown, init?: ResponseInit) {
  return new Response(JSON.stringify(data), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(init?.headers ?? {}),
    },
  });
}

function html(body: string, status = 200) {
  return new Response(body, {
    status,
    headers: {
      "content-type": "text/html; charset=utf-8",
    },
  });
}

function nowIso() {
  return new Date().toISOString();
}

function expiresAtIso() {
  return new Date(Date.now() + SESSION_TTL_SECONDS * 1000).toISOString();
}

function isExpired(expiresAt: string) {
  return Date.now() >= Date.parse(expiresAt);
}

function randomSecret() {
  const bytes = crypto.getRandomValues(new Uint8Array(18));
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function readSession(env: Env, sessionId: string) {
  const raw = await env.AUTH_SESSIONS.get(sessionId);
  return raw ? (JSON.parse(raw) as StoredSession) : null;
}

async function writeSession(env: Env, sessionId: string, session: StoredSession) {
  await env.AUTH_SESSIONS.put(sessionId, JSON.stringify(session), {
    expirationTtl: SESSION_TTL_SECONDS,
  });
}

async function deleteSession(env: Env, sessionId: string) {
  await env.AUTH_SESSIONS.delete(sessionId);
}

function slackOAuthAuthorizeUrl(env: Env, sessionId: string, sessionSecret: string) {
  const url = new URL("https://slack.com/oauth/v2/authorize");
  url.searchParams.set("client_id", env.SLACK_CLIENT_ID);
  url.searchParams.set("redirect_uri", env.SLACK_REDIRECT_URI);
  url.searchParams.set("state", `${sessionId}.${sessionSecret}`);
  url.searchParams.set("user_scope", USER_SCOPE);
  return url.toString();
}

async function exchangeCodeForToken(env: Env, code: string) {
  const credentials = btoa(`${env.SLACK_CLIENT_ID}:${env.SLACK_CLIENT_SECRET}`);
  const response = await fetch("https://slack.com/api/oauth.v2.access", {
    method: "POST",
    headers: {
      authorization: `Basic ${credentials}`,
      "content-type": "application/x-www-form-urlencoded",
    },
    body: new URLSearchParams({
      code,
      redirect_uri: env.SLACK_REDIRECT_URI,
    }),
  });
  const data = (await response.json()) as Record<string, any>;
  if (!data.ok) {
    throw new Error(data.error ?? "oauth_exchange_failed");
  }
  return data;
}

async function fetchSlackDisplayName(accessToken: string, slackUserId: string) {
  const url = new URL("https://slack.com/api/users.info");
  url.searchParams.set("user", slackUserId);
  const response = await fetch(url, {
    headers: {
      authorization: `Bearer ${accessToken}`,
    },
  });
  const data = (await response.json()) as Record<string, any>;
  if (!data.ok) {
    return "";
  }
  return (
    data.user?.profile?.display_name ||
    data.user?.profile?.real_name ||
    data.user?.name ||
    ""
  );
}

async function handleCreateSession(env: Env) {
  const sessionId = crypto.randomUUID();
  const sessionSecret = randomSecret();
  const expiresAt = expiresAtIso();
  await writeSession(env, sessionId, {
    status: "pending",
    sessionId,
    sessionSecret,
    expiresAt,
  });

  return json({
    sessionId,
    sessionSecret,
    authorizeUrl: slackOAuthAuthorizeUrl(env, sessionId, sessionSecret),
    expiresAt,
  });
}

async function handleGetSession(request: Request, env: Env, sessionId: string) {
  const sessionSecret = request.headers.get("x-session-secret")?.trim() ?? "";
  const session = await readSession(env, sessionId);
  if (!session || isExpired(session.expiresAt)) {
    if (session) {
      await deleteSession(env, sessionId);
    }
    return json({ status: "expired" });
  }

  if (session.sessionSecret !== sessionSecret) {
    return json({ status: "failed", error: "invalid_session_secret" }, { status: 403 });
  }

  if (session.status === "pending") {
    return json({
      status: "pending",
      expiresAt: session.expiresAt,
    });
  }

  if (session.status === "failed") {
    await deleteSession(env, sessionId);
    return json({
      status: "failed",
      error: session.error,
    });
  }

  await deleteSession(env, sessionId);
  return json({
    status: "completed",
    accessToken: session.accessToken,
    slackUserId: session.slackUserId,
    slackDisplayName: session.slackDisplayName,
    teamId: session.teamId,
    teamName: session.teamName,
    scope: session.scope,
  });
}

async function failCallback(
  env: Env,
  sessionId: string,
  sessionSecret: string,
  expiresAt: string,
  error: string,
) {
  await writeSession(env, sessionId, {
    status: "failed",
    sessionId,
    sessionSecret,
    expiresAt,
    error,
  });

  return html(
    `<!doctype html><html lang="ko"><body><h1>Slack 연결 실패</h1><p>${error}</p><p>앱으로 돌아가 다시 시도해주세요.</p></body></html>`,
    400,
  );
}

async function handleCallback(request: Request, env: Env) {
  const url = new URL(request.url);
  const state = url.searchParams.get("state") ?? "";
  const code = url.searchParams.get("code");
  const oauthError = url.searchParams.get("error");
  const [sessionId, sessionSecret] = state.split(".");

  if (!sessionId || !sessionSecret) {
    return html(
      "<!doctype html><html lang=\"ko\"><body><h1>잘못된 요청</h1><p>state 값이 올바르지 않습니다.</p></body></html>",
      400,
    );
  }

  const session = await readSession(env, sessionId);
  if (!session) {
    return html(
      "<!doctype html><html lang=\"ko\"><body><h1>세션이 만료되었어요</h1><p>앱으로 돌아가 다시 시도해주세요.</p></body></html>",
      400,
    );
  }

  if (session.sessionSecret !== sessionSecret || isExpired(session.expiresAt)) {
    await deleteSession(env, sessionId);
    return html(
      "<!doctype html><html lang=\"ko\"><body><h1>세션이 유효하지 않아요</h1><p>앱으로 돌아가 다시 시도해주세요.</p></body></html>",
      400,
    );
  }

  if (oauthError) {
    return failCallback(env, sessionId, sessionSecret, session.expiresAt, oauthError);
  }

  if (!code) {
    return failCallback(
      env,
      sessionId,
      sessionSecret,
      session.expiresAt,
      "missing_code",
    );
  }

  try {
    const tokenData = await exchangeCodeForToken(env, code);
    const accessToken =
      tokenData.authed_user?.access_token || tokenData.access_token || "";
    const slackUserId =
      tokenData.authed_user?.id || tokenData.authed_user?.user_id || tokenData.user_id || "";
    const teamId = tokenData.team?.id || tokenData.enterprise?.id || "";
    const teamName = tokenData.team?.name || tokenData.enterprise?.name || "";
    const scope = tokenData.authed_user?.scope || tokenData.scope || USER_SCOPE;

    if (!accessToken || !slackUserId || !teamId) {
      throw new Error("missing_required_token_fields");
    }

    const slackDisplayName = await fetchSlackDisplayName(accessToken, slackUserId);
    await writeSession(env, sessionId, {
      status: "completed",
      sessionId,
      sessionSecret,
      expiresAt: session.expiresAt,
      accessToken,
      slackUserId,
      slackDisplayName,
      teamId,
      teamName,
      scope,
    });

    return html(
      `<!doctype html><html lang="ko"><body><h1>Slack 연결 완료</h1><p>${teamName || teamId} / ${slackDisplayName || slackUserId}</p><p>앱으로 돌아가면 연결이 완료됩니다.</p><p>${nowIso()}</p></body></html>`,
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return failCallback(env, sessionId, sessionSecret, session.expiresAt, message);
  }
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const { pathname } = url;

    if (request.method === "POST" && pathname === "/slack/auth/session") {
      return handleCreateSession(env);
    }

    if (request.method === "GET" && pathname.startsWith("/slack/auth/session/")) {
      const sessionId = pathname.split("/").pop() ?? "";
      return handleGetSession(request, env, sessionId);
    }

    if (request.method === "GET" && pathname === "/slack/oauth/callback") {
      return handleCallback(request, env);
    }

    return json({ error: "not_found" }, { status: 404 });
  },
};
