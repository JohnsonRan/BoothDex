import {
  afterEach,
  beforeEach,
  expect,
  jest,
  mock,
  spyOn,
  test,
} from "bun:test";
import { createDedupeFetch } from "next/dist/server/lib/dedupe-fetch";
import * as request from "next/headers";
import { DEFAULT_FILTERS } from "./catalog";
import { searchItems } from "./server-data";

const nextTurn = setImmediate;
const turn = () => new Promise<void>((resolve) => nextTurn(resolve));
const original = globalThis.fetch;
const paths: string[] = [];
let searchSignal: AbortSignal | null | undefined;

beforeEach(() => {
  jest.useFakeTimers();
  paths.length = 0;
  spyOn(request, "headers").mockResolvedValue(
    new Headers() as Awaited<ReturnType<typeof request.headers>>,
  );
});
afterEach(() => {
  mock.restore();
  globalThis.fetch = original;
  jest.useRealTimers();
});

function search(
  reply: Response | Promise<Response>,
  health: (init?: RequestInit) => Response | Promise<Response> = () =>
    new Response(null),
) {
  const localFetch = (async (url: string, init?: RequestInit) => {
    const path = new URL(url).pathname;
    paths.push(path);
    if (path === "/api/health") return health(init);
    searchSignal = init?.signal;
    return reply;
  }) as typeof fetch;
  globalThis.fetch = createDedupeFetch(localFetch) as typeof fetch;
  return searchItems(DEFAULT_FILTERS, 1, false);
}

function stalled503(type: string) {
  const cancel = mock(() => new Promise<void>(() => {}));
  const res = new Response(new ReadableStream({ cancel }), {
    status: 503,
    headers: { "content-type": type },
  });
  return { res, cancel };
}

test("a known non-JSON 503 checks health without reading or waiting for its body", async () => {
  const { res, cancel } = stalled503("text/html; charset=utf-8");
  const json = spyOn(res, "json");
  const outcome = search(res);
  await turn();
  expect(json).not.toHaveBeenCalled();
  expect(paths).toEqual(["/api/search", "/api/health"]);
  expect(await outcome).toEqual({ state: "error" });
  expect(cancel).toHaveBeenCalledTimes(1);
  expect(jest.getTimerCount()).toBe(0);
});

test("a proxy's text/plain 503 can still carry the explicit startup reply", async () => {
  const outcome = search(
    new Response('{"starting":true}', {
      status: 503,
      headers: {
        "Content-Type": "Text/Plain; charset=utf-8",
        "Retry-After": "9",
      },
    }),
  );
  expect(await outcome).toEqual({ state: "starting", retryAfterMs: 9000 });
  expect(paths).toEqual(["/api/search"]);
  expect(jest.getTimerCount()).toBe(0);
});

test.each(["application/json", "text/plain"])(
  "a stalled %s 503 releases its body and a stalled fallback health probe times out",
  async (type) => {
    const { res, cancel } = stalled503(type);
    const outcome = search(
      res,
      (init) =>
        new Promise<Response>((_, reject) =>
          init?.signal?.addEventListener("abort", () =>
            reject(init.signal?.reason),
          ),
        ),
    );
    await turn();
    expect(paths).toEqual(["/api/search"]);
    jest.advanceTimersByTime(20_000);
    await turn();
    expect(cancel).toHaveBeenCalledTimes(1);
    expect(paths).toEqual(["/api/search", "/api/health"]);
    jest.advanceTimersByTime(20_000);
    await turn();
    expect(await outcome).toEqual({ state: "starting", retryAfterMs: 3000 });
    expect(jest.getTimerCount()).toBe(0);
  },
);

test("a slow successful search can finish without a deadline on its headers or body", async () => {
  const { promise, resolve } = Promise.withResolvers<Response>();
  const stream = new TransformStream();
  const writer = stream.writable.getWriter();
  const outcome = search(promise);
  await turn();
  jest.advanceTimersByTime(60_000);
  resolve(new Response(stream.readable));
  await turn();
  jest.advanceTimersByTime(60_000);
  expect(searchSignal?.aborted).not.toBe(true);
  await writer.write(new TextEncoder().encode('{"count":1}'));
  await writer.close();
  expect(await outcome).toMatchObject({ state: "ok", result: { count: 1 } });
  expect(paths).toEqual(["/api/search"]);
  expect(jest.getTimerCount()).toBe(0);
});
