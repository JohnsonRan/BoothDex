import { expect, test } from "bun:test";
import { EARLY_CLICKS } from "./early-clicks";

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

// A document and one control, as far as the script and React's capture listener see them. React listens once
// `listen()` is called; it handles a click when the control's code is `ready` (hydrating it on the spot, as React
// does for a discrete event) and otherwise stops the event in the capture phase, as React does.
function page() {
  const listeners: ((e: unknown) => void)[] = [];
  const doc: Record<string, unknown> = {
    addEventListener: (_: string, fn: (e: unknown) => void) => {
      listeners.push(fn);
    },
  };
  const control: Record<string, unknown> & { ready: boolean } = {
    isConnected: true,
    ready: false,
    handled: 0,
    closest: () => control,
    click: () => dispatch(),
  };
  const react = (e: { stopPropagation: () => void }) => {
    if (!doc._reactListening1) return;
    if (!control.ready) return e.stopPropagation();
    control.__reactProps1 = {};
    control.handled = (control.handled as number) + 1;
  };
  function dispatch() {
    const e = {
      target: control,
      eventPhase: 1,
      currentTarget: doc,
      stopped: false,
      stopPropagation() {
        e.stopped = true;
      },
    };
    for (const fn of [...listeners, react]) fn(e);
  }
  new Function("document", EARLY_CLICKS)(doc);
  const listen = () => {
    doc._reactListening1 = true;
  };
  return { control, userClick: dispatch, listen };
}

test("a click before React listens is replayed once, also after several clicks", async () => {
  const { control, userClick, listen } = page();
  userClick();
  userClick();
  await wait(120);
  expect(control.handled).toBe(0);
  control.ready = true;
  listen();
  await wait(150);
  expect(control.handled).toBe(1);
  await wait(400);
  expect(control.handled).toBe(1);
});

test("a click React drops (code not loaded yet) is replayed once the control is hydrated", async () => {
  const { control, userClick, listen } = page();
  listen();
  userClick();
  await wait(400);
  expect(control.handled).toBe(0);
  // React hydrates the control on its own
  control.ready = true;
  control.__reactProps1 = {};
  await wait(400);
  expect(control.handled).toBe(1);
  await wait(400);
  expect(control.handled).toBe(1);
});

test("clicks React handles are left alone", async () => {
  const { control, userClick, listen } = page();
  listen();
  control.ready = true;
  userClick();
  await wait(400);
  expect(control.handled).toBe(1);
});
