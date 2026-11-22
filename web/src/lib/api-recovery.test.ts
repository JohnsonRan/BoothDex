import { afterEach, beforeEach, expect, jest, mock, test } from "bun:test";
import { retryAfterMs } from "./api";
import { watchApiRecovery } from "./client-data";

const nextTurn = setImmediate;
const original = globalThis.fetch;
const calls: {
  path: string;
  signal?: AbortSignal | null;
  answer: (r: Response) => void;
  reject: (reason: unknown) => void;
}[] = [];
const cleanups: (() => void)[] = [];

beforeEach(() => {
  jest.useFakeTimers();
  calls.length = 0;
  globalThis.fetch = ((path: string, init?: RequestInit) =>
    new Promise<Response>((answer, reject) =>
      calls.push({ path, signal: init?.signal, answer, reject }),
    )) as typeof fetch;
});
afterEach(() => {
  for (const stop of cleanups.splice(0)) stop();
  globalThis.fetch = original;
  jest.useRealTimers();
});

function subscribe(reload: () => void, delay = 5000) {
  const stop = watchApiRecovery(reload, delay);
  cleanups.push(stop);
  return stop;
}

async function answer(i: number, status: number, retryAfter?: string) {
  calls[i].answer(
    new Response(null, {
      status,
      headers: retryAfter ? { "Retry-After": retryAfter } : {},
    }),
  );
  await new Promise<void>((resolve) => nextTurn(resolve));
}

test("an HTTP-date delays the next probe; an absent or unusable header keeps the normal delay", () => {
  jest.setSystemTime(new Date("2030-01-01T00:00:00Z"));
  const delay = (value: string) =>
    retryAfterMs(new Response(null, { headers: { "Retry-After": value } }));
  expect(delay("Tue, 01 Jan 2030 00:00:09 GMT")).toBe(9000);
  expect(delay("later")).toBe(3000);
  expect(retryAfterMs()).toBe(3000);
});

test("startup views share one slow probe, follow Retry-After and reload once for the tab", async () => {
  const reload = mock(() => {});
  const stale = mock(() => {});
  const stop = subscribe(stale);
  subscribe(reload);
  subscribe(reload);
  jest.advanceTimersByTime(4999);
  expect(calls).toHaveLength(0);
  jest.advanceTimersByTime(1);
  expect(calls.map((c) => c.path)).toEqual(["/api/health"]);
  jest.advanceTimersByTime(1000);
  expect(calls).toHaveLength(1);
  stop();
  expect(calls[0].signal?.aborted).toBe(false);
  await answer(0, 503, "7");
  jest.advanceTimersByTime(6999);
  expect(calls).toHaveLength(1);
  jest.advanceTimersByTime(1);
  expect(calls).toHaveLength(2);
  expect(reload).not.toHaveBeenCalled();
  await answer(1, 200);
  jest.advanceTimersByTime(20_000);
  expect(calls).toHaveLength(2);
  expect(stale).not.toHaveBeenCalled();
  expect(reload).toHaveBeenCalledTimes(1);
});

test("a hung probe times out, then the remaining subscriber gets one retry", async () => {
  const stale = mock(() => {});
  const reload = mock(() => {});
  const stop = subscribe(stale);
  subscribe(reload);
  jest.advanceTimersByTime(5000);
  expect(calls).toHaveLength(1);
  stop();
  const hung = calls[0];
  hung.signal?.addEventListener("abort", () =>
    hung.reject(hung.signal?.reason),
  );
  jest.advanceTimersByTime(3000);
  await new Promise<void>((resolve) => nextTurn(resolve));
  expect(hung.signal?.aborted).toBe(true);
  expect(calls).toHaveLength(1);
  expect(reload).not.toHaveBeenCalled();
  jest.advanceTimersByTime(2999);
  expect(calls).toHaveLength(1);
  jest.advanceTimersByTime(1);
  expect(calls).toHaveLength(2);
  expect(calls[1].signal?.aborted).toBe(false);
  const cancel = mock(() => {});
  calls[1].answer(new Response(new ReadableStream({ cancel })));
  await new Promise<void>((resolve) => nextTurn(resolve));
  expect(cancel).toHaveBeenCalledTimes(1);
  expect(reload).toHaveBeenCalledTimes(1);
  expect(stale).not.toHaveBeenCalled();
  expect(jest.getTimerCount()).toBe(0);
});

test("the last unmount cancels the timer or flight; a late answer cannot reload a new view", async () => {
  const stale = mock(() => {});
  subscribe(stale)();
  jest.advanceTimersByTime(20_000);
  expect(calls).toHaveLength(0);
  const stop = subscribe(stale);
  jest.advanceTimersByTime(5000);
  stop();
  expect(calls[0].signal?.aborted).toBe(true);
  const current = mock(() => {});
  subscribe(current);
  await answer(0, 200);
  expect(stale).not.toHaveBeenCalled();
  expect(current).not.toHaveBeenCalled();
  jest.advanceTimersByTime(5000);
  await answer(1, 200);
  expect(current).toHaveBeenCalledTimes(1);
});
