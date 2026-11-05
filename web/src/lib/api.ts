// Same-origin calls to the Rust API (proxied by next.config rewrites).

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

/** False while the API is still loading the catalogue (it answers 503) or not reachable at all. */
export const apiReady = () =>
  fetch("/api/health", { cache: "no-store" }).then(
    (r) => r.ok,
    () => false,
  );

export async function api<T>(
  path: string,
  init?: RequestInit & { json?: unknown },
): Promise<T> {
  const { json, ...rest } = init ?? {};
  const res = await fetch(path, {
    ...rest,
    headers:
      json === undefined
        ? rest.headers
        : { "content-type": "application/json", ...rest.headers },
    body: json === undefined ? rest.body : JSON.stringify(json),
    credentials: "same-origin",
  });
  const body = res.headers.get("content-type")?.includes("json")
    ? await res.json()
    : null;
  if (!res.ok) throw new ApiError(res.status, body?.error ?? res.statusText);
  return body as T;
}

let accountRequest: Promise<unknown> | null = null;
/** `/api/account`, shared by callers asking at the same time (settings and the session both read it on load). */
export function fetchAccount<T>(options?: { fresh?: boolean }): Promise<T> {
  if (options?.fresh) accountRequest = null;
  if (!accountRequest) {
    const p: Promise<unknown> = api<T>("/api/account").finally(() => {
      if (accountRequest === p) accountRequest = null;
    });
    accountRequest = p;
  }
  return accountRequest as Promise<T>;
}

export const fmt = {
  num: (n: number | null | undefined) =>
    n == null ? "—" : n.toLocaleString("zh-CN"),
  bytes(n: number | null | undefined) {
    if (n == null) return "—";
    const u = ["B", "KB", "MB", "GB", "TB"];
    let i = 0;
    let v = n;
    while (v >= 1024 && i < u.length - 1) {
      v /= 1024;
      i++;
    }
    return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${u[i]}`;
  },
  ago(iso: string | null | undefined, now = Date.now()) {
    if (!iso) return "—";
    const s = Math.max(0, Math.round((now - Date.parse(iso)) / 1000));
    if (s < 60) return `${s} 秒前`;
    if (s < 3600) return `${Math.floor(s / 60)} 分钟前`;
    if (s < 86400) return `${Math.floor(s / 3600)} 小时前`;
    return `${Math.floor(s / 86400)} 天前`;
  },
  duration(fromIso: string | null | undefined, toIso?: string | null) {
    if (!fromIso) return "—";
    const s = Math.round(
      ((toIso ? Date.parse(toIso) : Date.now()) - Date.parse(fromIso)) / 1000,
    );
    if (s < 60) return `${s} 秒`;
    if (s < 3600) return `${Math.floor(s / 60)} 分 ${s % 60} 秒`;
    return `${Math.floor(s / 3600)} 小时 ${Math.floor((s % 3600) / 60)} 分`;
  },
};
