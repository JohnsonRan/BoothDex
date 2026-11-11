import { expect, test } from "bun:test";
import { EARLY_CLICKS } from "./early-clicks";

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

type Control = Record<string, unknown> & { ready: boolean; handled: number };

// A document and one control, as far as the script and React's capture listener see them. React listens once
// `listen()` is called; it handles a click when the control's code is `ready` (hydrating it on the spot, as React
// does for a discrete event) and otherwise stops the event in the capture phase, as React does; an `ignored`
// control's clicks pass with no handler. `replace` swaps the control for one React has drawn, carrying `late` as its
// `data-late-menu`, as a client render of a late menu does.
function page(late?: string) {
  const listeners: ((e: unknown) => void)[] = [];
  let current: Control;
  const doc: Record<string, unknown> = {
    addEventListener: (_: string, fn: (e: unknown) => void) => {
      listeners.push(fn);
    },
    querySelector: (s: string) =>
      late && s === `[data-late-menu="${late}"]` ? current : null,
  };
  const make = (): Control => {
    const c: Control = {
      isConnected: true,
      ready: false,
      handled: 0,
      closest: () => c,
      getAttribute: (name: string) =>
        name === "data-late-menu" ? (late ?? null) : null,
      hasAttribute: (name: string) => name === "data-late-menu" && !!late,
      click: () => dispatch(c),
    };
    return c;
  };
  current = make();
  const react = (e: { target: Control; stopPropagation: () => void }) => {
    if (!doc._reactListening1 || e.target.ignored) return;
    if (!e.target.ready) return e.stopPropagation();
    e.target.__reactProps1 = {};
    e.target.handled += 1;
  };
  function dispatch(target: Control) {
    const e = {
      target,
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
  const replace = () => {
    current.isConnected = false;
    current = make();
    current.ready = true;
    current.__reactProps1 = {};
    return current;
  };
  const control = current;
  return { control, userClick: () => dispatch(control), listen, replace };
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

test("a late menu's button drawn again by the client gets the click kept on the server's", async () => {
  const { control, userClick, listen, replace } = page("currency");
  userClick();
  // React listens before it draws the menu's boundary again, and lets a click on the server's button pass
  control.ignored = true;
  listen();
  await wait(400);
  const drawn = replace();
  await wait(400);
  expect(control.handled).toBe(0);
  expect(drawn.handled).toBe(1);
  await wait(400);
  expect(drawn.handled).toBe(1);
});

test("a replaced control without a late menu drops its click", async () => {
  const { userClick, listen, replace } = page();
  userClick();
  const drawn = replace();
  listen();
  await wait(400);
  expect(drawn.handled).toBe(0);
});

test("clicks React handles are left alone", async () => {
  const { control, userClick, listen } = page();
  listen();
  control.ready = true;
  userClick();
  await wait(400);
  expect(control.handled).toBe(1);
});
