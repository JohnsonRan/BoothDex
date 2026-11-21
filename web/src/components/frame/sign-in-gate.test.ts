import { expect, test } from "bun:test";
import {
  askRegistration,
  type Registration,
  tokenFor,
  waitForAnswer,
} from "./sign-in-gate";

const checked: Registration = { open: true, site_key: "0x-site" };
const unchecked: Registration = { open: true, site_key: null };

test("a submit sends a token only once the answer and the widget allow it", () => {
  // the answer names a site key and the widget is not drawn yet, or has no token: nothing is sent
  expect(tokenFor(checked, null)).toBe("required");
  expect(tokenFor(checked, { token: "", failed: false })).toBe("required");
  expect(tokenFor(checked, { token: "t", failed: false })).toBe("t");
  // a widget whose script did not load, no site key, and no answer: the API decides
  expect(tokenFor(checked, { token: "", failed: true })).toBe("");
  expect(tokenFor(unchecked, null)).toBe("");
  expect(tokenFor(null, null)).toBe("");
});

test("an ask gives up on a failed or late answer", async () => {
  expect(await askRegistration(() => Promise.resolve(checked), 50)).toEqual(
    checked,
  );
  expect(
    await askRegistration(() => Promise.reject(new Error("offline")), 50),
  ).toBeNull();
  const late = () =>
    new Promise<Registration>((resolve) =>
      setTimeout(() => resolve(checked), 200),
    );
  expect(await askRegistration(late, 20)).toBeNull();
});

test("a submit waits for the answer of this open, and asks once more after a failed one", async () => {
  let release: (a: Registration) => void = () => {};
  const pending = new Promise<Registration | null>((resolve) => {
    release = resolve;
  });
  let asked = 0;
  const ask = () => {
    asked++;
    return Promise.resolve(checked);
  };
  const waiting = waitForAnswer({ pending, ask, still: () => true });
  // the submit started before the answer goes by it once it arrives
  setTimeout(() => release(checked), 10);
  expect(await waiting).toEqual({ answer: checked });
  expect(asked).toBe(0);
  // a failed first answer is asked again once
  expect(
    await waitForAnswer({
      pending: Promise.resolve(null),
      ask,
      still: () => true,
    }),
  ).toEqual({ answer: checked });
  expect(asked).toBe(1);
  // both failed: the submit goes on without an answer
  expect(
    await waitForAnswer({
      pending: Promise.resolve(null),
      ask: () => Promise.resolve(null),
      still: () => true,
    }),
  ).toEqual({ answer: null });
});

test("a submit whose form changed while it waited is cancelled", async () => {
  let shown = true;
  const pending = new Promise<Registration | null>((resolve) =>
    setTimeout(() => {
      // the dialog closed, opened again or changed mode meanwhile
      shown = false;
      resolve(checked);
    }, 10),
  );
  expect(
    await waitForAnswer({
      pending,
      ask: () => Promise.resolve(checked),
      still: () => shown,
    }),
  ).toBe("cancelled");
  shown = true;
  const changedDuringAsk = await waitForAnswer({
    pending: Promise.resolve(null),
    ask: () => {
      shown = false;
      return Promise.resolve(checked);
    },
    still: () => shown,
  });
  expect(changedDuringAsk).toBe("cancelled");
});
