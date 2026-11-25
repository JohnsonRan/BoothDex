import { expect, test } from "bun:test";
import {
  askRegistration,
  createSubmitFlight,
  type Registration,
  tokenFor,
  waitForAnswer,
} from "./sign-in-gate";

const checked: Registration = { open: true, site_key: "0x-site" };
const unchecked: Registration = { open: true, site_key: null };

test("a submit sends a token only once the answer and the widget allow it", () => {
  // the answer names a site key and the widget is not drawn yet, or has no token: nothing is sent
  expect(tokenFor(checked, null)).toBe("required");
  expect(
    tokenFor(checked, { siteKey: "0x-site", token: "", failed: false }),
  ).toBe("required");
  expect(
    tokenFor(checked, { siteKey: "0x-site", token: "t", failed: false }),
  ).toBe("t");
  // a widget drawn for an earlier site key (rotated between opens) never answers for the new one
  expect(
    tokenFor(checked, { siteKey: "0x-old", token: "t", failed: false }),
  ).toBe("required");
  expect(
    tokenFor(checked, { siteKey: "0x-old", token: "", failed: true }),
  ).toBe("required");
  // a widget whose script did not load, no site key, and no answer: the API decides
  expect(
    tokenFor(checked, { siteKey: "0x-site", token: "", failed: true }),
  ).toBe("");
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

test("form change cancels a registration wait without blocking the next submit", async () => {
  const flight = createSubmitFlight();
  let releaseWait: (r: Registration) => void = () => {};
  const pending = new Promise<Registration | null>((resolve) => {
    releaseWait = resolve;
  });
  let posts = 0;
  let busy = false;

  const a1 = flight.begin();
  expect(a1).not.toBeNull();
  if (!a1) return;
  busy = true;
  // double-submit while waiting is ignored
  expect(flight.begin()).toBeNull();

  const wait1 = waitForAnswer({
    pending,
    ask: () => Promise.resolve(unchecked),
    still: a1.still,
  });

  // closing/reopening or switching tabs while still waiting cancels the wait and unlocks busy
  busy = flight.onFormChange();
  expect(busy).toBe(false);

  // second submit starts cleanly and enters POST
  const a2 = flight.begin();
  expect(a2).not.toBeNull();
  if (!a2) return;
  busy = true;
  expect(a2.enterPost()).toBe(true);
  posts++;

  // first wait settles late: cancelled, cannot enter POST, and finish() does not clear a2's busy state
  releaseWait(unchecked);
  expect(await wait1).toBe("cancelled");
  expect(a1.enterPost()).toBe(false);
  expect(a1.finish()).toBe(false);
  expect(busy).toBe(true);
  expect(posts).toBe(1);

  // a2 finishes cleanly
  expect(a2.finish()).toBe(true);
  busy = false;
  expect(busy).toBe(false);
});

test("reopening after an account POST starts keeps the POST single-flight until it settles", () => {
  const flight = createSubmitFlight();
  const a1 = flight.begin();
  expect(a1).not.toBeNull();
  if (!a1) return;
  expect(a1.enterPost()).toBe(true);

  // close and reopen while the account POST is still pending
  expect(flight.onFormChange()).toBe(true);
  expect(flight.onFormChange()).toBe(true);
  // a1 is no longer on the same form (no stale error toast or widget reset),
  // but no second account POST may overlap until a1 settles
  expect(a1.still()).toBe(false);
  expect(flight.begin()).toBeNull();

  // once the single in-flight POST settles, the reopened form unlocks for a new submit
  expect(a1.finish()).toBe(true);
  const a2 = flight.begin();
  expect(a2).not.toBeNull();
  expect(a2?.enterPost()).toBe(true);
  expect(a2?.finish()).toBe(true);
});
