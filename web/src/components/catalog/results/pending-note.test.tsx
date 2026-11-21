import { expect, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import { renderToStaticMarkup } from "react-dom/server";
import { MESSAGES } from "@/lib/dictionaries";
import { armRetry, PendingNote } from "./result-states";

const { en } = MESSAGES;

test("a pending search is read once more after a moment, and only once", async () => {
  let reads = 0;
  let asked = false;
  const read = () => {
    reads += 1;
    asked = true;
  };
  expect(armRetry(asked, false, read, 5)).toBeFunction();
  await Bun.sleep(30);
  expect(reads).toBe(1);
  // the answer to that read is pending again: the note, now asked, arms nothing
  expect(armRetry(asked, false, read, 5)).toBeUndefined();
  await Bun.sleep(30);
  expect(reads).toBe(1);
});

test("the read waits a second, or the time it is given", () => {
  const delays: (number | undefined)[] = [];
  const real = globalThis.setTimeout;
  globalThis.setTimeout = ((_: () => void, ms?: number) => {
    delays.push(ms);
    return 0;
  }) as typeof setTimeout;
  try {
    armRetry(false, false, () => {});
    armRetry(false, false, () => {}, 5);
  } finally {
    globalThis.setTimeout = real;
  }
  expect(delays).toEqual([1000, 5]);
});

test("a navigation, or a new search that drops the note, cancels the read", async () => {
  let reads = 0;
  const read = () => {
    reads += 1;
  };
  expect(armRetry(false, true, read, 5)).toBeUndefined();
  armRetry(false, false, read, 5)?.();
  await Bun.sleep(30);
  expect(reads).toBe(0);
});

test("the note says the meaning-based matches are on their way until the read is answered", () => {
  const html = renderToStaticMarkup(
    <AppRouterContext value={{} as AppRouterInstance}>
      <PendingNote
        pending={en["list.semanticPending"]}
        unavailable={en["list.semanticUnavailable"]}
      />
    </AppRouterContext>,
  );
  expect(html).toContain(en["list.semanticPending"]);
  expect(html).not.toContain(en["list.semanticUnavailable"]);
});
