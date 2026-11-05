// Same-origin calls to the Rust API (proxied by next.config rewrites).

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

/**
 * The API errors a catalogue explains in its own words. The API identifies an error only by its English
 * message, so `messages` matches that exactly; `statuses` covers the rest by HTTP status.
 */
export type ApiErrors<K extends string> = {
  messages: Readonly<Record<string, K>>;
  statuses: Readonly<Partial<Record<number, K>>>;
};

/** The message key for a failed call: what `known` names its error, else `fallback` (a network failure too). */
export function errorKey<K extends string>(
  e: unknown,
  known: ApiErrors<K>,
  fallback: K,
): K {
  if (!(e instanceof ApiError)) return fallback;
  return (
    (Object.hasOwn(known.messages, e.message)
      ? known.messages[e.message]
      : known.statuses[e.status]) ?? fallback
  );
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
