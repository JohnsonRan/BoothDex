import { afterEach, expect, jest, test } from "bun:test";
import { LOAD_TIMEOUT_MS, loadTurnstile } from "./turnstile";

type Script = {
  onload: (() => void) | null;
  onerror: (() => void) | null;
  remove: () => void;
};

const g = globalThis as Record<string, unknown>;

// A document whose head only collects the scripts added to it; `window` is the global object, as in a browser.
function page() {
  const head: Script[] = [];
  g.window = globalThis;
  g.document = {
    createElement: () => {
      const s: Script = {
        onload: null,
        onerror: null,
        remove: () => head.splice(head.indexOf(s), 1),
      };
      return s;
    },
    head: { append: (s: Script) => head.push(s) },
  };
  return head;
}

afterEach(() => {
  jest.useRealTimers();
  delete g.turnstile;
  delete g.document;
  delete g.window;
});

// `loadTurnstile` keeps its promise for the page's life, so one test walks the whole sequence.
test("a script that never answers fails after the timeout; Try again uses a late arrival", async () => {
  jest.useFakeTimers();
  const head = page();
  let outcome = "pending";
  loadTurnstile().then(
    () => {
      outcome = "loaded";
    },
    (e: Error) => {
      outcome = e.message;
    },
  );
  const stalled = head[0];
  jest.advanceTimersByTime(LOAD_TIMEOUT_MS);
  for (let i = 0; i < 5; i++) await null;
  expect(outcome).toBe("Turnstile timed out");
  expect(head).toHaveLength(0);

  // the stalled script arrives after all and starts Turnstile; its load no longer reaches the form
  expect(stalled.onload).toBeNull();
  const api = { render: () => "w", reset: () => {}, remove: () => {} };
  g.turnstile = api;
  let retried: unknown;
  loadTurnstile().then((t) => {
    retried = t;
  });
  for (let i = 0; i < 5; i++) await null;
  expect(retried).toBe(api);
  expect(head).toHaveLength(0);
});
