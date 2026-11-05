import { expect, test } from "bun:test";
import { EARLY_CLICKS } from "./early-clicks";

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

// a document and one control, as far as the script reads them; `ready` is whether React can hydrate it when
// clicked (its code has loaded), and a handled click is one that reaches a hydrated control
function page() {
  const doc: Record<string, unknown> & { click?: (e: unknown) => void } = {
    addEventListener: (_: string, fn: (e: unknown) => void) => {
      doc.click = fn;
    },
  };
  const control: Record<string, unknown> = {
    isConnected: true,
    ready: false,
    calls: 0,
    handled: 0,
    closest: () => control,
    click() {
      control.calls = (control.calls as number) + 1;
      if (doc._reactListening1 && control.ready) {
        control.__reactProps1 = {};
        control.handled = (control.handled as number) + 1;
      }
    },
  };
  new Function("document", EARLY_CLICKS)(doc);
  const userClick = () => doc.click?.({ target: control });
  const listen = () => {
    doc._reactListening1 = true;
  };
  return { control, userClick, listen };
}

test("an early click is replayed once React listens, also after several clicks", async () => {
  const { control, userClick, listen } = page();
  userClick();
  userClick();
  await wait(120);
  expect(control.calls).toBe(0);
  control.ready = true;
  listen();
  await wait(120);
  expect(control.handled).toBe(1);
  expect(control.calls).toBe(1);
});

test("when the clicked part's code is not ready, the click is repeated once it is hydrated", async () => {
  const { control, userClick, listen } = page();
  userClick();
  listen();
  await wait(120);
  expect(control.calls).toBe(1);
  expect(control.handled).toBe(0);
  // React hydrates the control on its own
  control.__reactProps1 = {};
  control.ready = true;
  await wait(120);
  expect(control.handled).toBe(1);
  await wait(120);
  expect(control.calls).toBe(2);
});

test("clicks after React listens are left to React", async () => {
  const { control, userClick, listen } = page();
  listen();
  control.ready = true;
  userClick();
  await wait(120);
  expect(control.calls).toBe(0);
});
