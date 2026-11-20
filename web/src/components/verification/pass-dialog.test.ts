import { afterEach, expect, test } from "bun:test";
import { verify } from "./pass-dialog";

const realFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = realFetch;
});

/** `fetch` answering `status` with `body` and `headers`, recording what it was sent. */
function answer(status: number, body: object, headers: HeadersInit = {}) {
  const sent: { url: string; init?: RequestInit }[] = [];
  globalThis.fetch = (async (url: string, init?: RequestInit) => {
    sent.push({ url, init });
    return new Response(JSON.stringify(body), {
      status,
      headers: { "content-type": "application/json", ...headers },
    });
  }) as typeof fetch;
  return sent;
}

const t = (key: string) => key;

test("the stricter limits' check still sends its token to /api/verify", async () => {
  const sent = answer(200, { ok: true });
  expect(await verify("tok", t)).toBeNull();
  expect(sent.map((s) => [s.url, s.init?.method, s.init?.body])).toEqual([
    ["/api/verify", "POST", JSON.stringify({ token: "tok" })],
  ]);
  answer(403, { code: "verification_failed" });
  expect(await verify("tok", t)).toEqual({ message: "refused", retryAfter: 0 });
  answer(429, { code: "x" }, { "retry-after": "60" });
  expect(await verify("tok", t)).toEqual({
    message: "unavailable",
    retryAfter: 60,
  });
});
