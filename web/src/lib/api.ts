// Same-origin calls to the Rust API (proxied by next.config rewrites).

import type { Refusal } from "./api-types";

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    /** why the API refused; none for a rate limit, a failure inside the API or an answer that is not the API's */
    public code?: Refusal,
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
  if (!res.ok)
    throw new ApiError(res.status, body?.error ?? res.statusText, body?.code);
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
