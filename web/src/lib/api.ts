// Same-origin calls to the Rust API (proxied by next.config rewrites).

import type { Refusal } from "./api-types";

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    /** why the API refused; none for a rate limit, a failure inside the API or an answer that is not the API's */
    public code?: Refusal,
    /** the site key of the check the refusal asks for (`verification_offered`, `verification_required`) */
    public siteKey?: string,
  ) {
    super(message);
  }
}

/**
 * The API errors a catalogue explains in its own words: `codes` by the refusal the API names (crates/api/src/error.rs),
 * `statuses` for the rest by HTTP status.
 */
export type ApiErrors<K extends string> = {
  codes: Readonly<Partial<Record<Refusal, K>>>;
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
    (e.code && Object.hasOwn(known.codes, e.code)
      ? known.codes[e.code]
      : known.statuses[e.status]) ?? fallback
  );
}

export type ApiStarting = { state: "starting"; retryAfterMs: number };

/** The startup listener sends Retry-After in seconds. A proxy may send an HTTP date instead. */
export function retryAfterMs(response?: Response): number {
  const value = response?.headers.get("retry-after") ?? "";
  const ms = /^\d+$/.test(value)
    ? Number(value) * 1000
    : /^[A-Za-z]{3}, /.test(value)
      ? Date.parse(value) - Date.now()
      : 0;
  return ms > 0 ? Math.min(ms, 2_147_483_647) : 3000;
}

/** Null when healthy, otherwise the delay before checking again. */
export async function probeHealth(origin: string, signal?: AbortSignal) {
  const res = await fetch(`${origin}/api/health`, {
    cache: "no-store",
    signal,
  }).catch(() => undefined);
  return res?.ok ? null : retryAfterMs(res);
}

/** The check that lifts the stricter rate limits, set by components/verification/pass-host.tsx: true once it passed,
 *  false when the visitor closed it (and before the host is mounted). */
export const passCheck = {
  open: (_siteKey: string) => Promise.resolve(false),
};

/** A server read refused by the stricter limit (lib/server-data.ts): the page offers the check (catalog/refresh.tsx). */
export type PassOffer = { siteKey: string };

type Init = RequestInit & { json?: unknown };

export function api<T>(path: string, init?: Init): Promise<T> {
  return send<T>(path, init, true);
}

async function send<T>(
  path: string,
  init: Init | undefined,
  offer: boolean,
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
  if (!res.ok) {
    // the stricter limit of a visitor without a pass (crates/api/src/pass.rs): sent once more after a passed check
    if (
      offer &&
      body?.code === "verification_offered" &&
      (await passCheck.open(body.site_key))
    )
      return send<T>(path, init, false);
    throw new ApiError(
      res.status,
      body?.error ?? res.statusText,
      body?.code,
      body?.site_key,
    );
  }
  return body as T;
}

let accountRequest: Promise<unknown> | null = null;
/** `/api/account`, null signed out; shared by callers asking at the same time (the session and the home page's
 *  avatar both read it on load). */
export function fetchAccount<T>(options?: {
  fresh?: boolean;
}): Promise<T | null> {
  if (options?.fresh) accountRequest = null;
  if (!accountRequest) {
    const p: Promise<unknown> = api<T>("/api/account").finally(() => {
      if (accountRequest === p) accountRequest = null;
    });
    accountRequest = p;
  }
  return accountRequest as Promise<T | null>;
}
