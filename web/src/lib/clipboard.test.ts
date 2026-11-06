import { afterEach, expect, test } from "bun:test";
import { copyText } from "./clipboard";

const realNavigator = globalThis.navigator;
const withClipboard = (clipboard: unknown) =>
  Object.defineProperty(globalThis, "navigator", {
    value: { clipboard },
    configurable: true,
  });
afterEach(() => {
  Object.defineProperty(globalThis, "navigator", {
    value: realNavigator,
    configurable: true,
  });
});

test("copies through the clipboard", async () => {
  const written: string[] = [];
  withClipboard({ writeText: async (s: string) => void written.push(s) });
  await copyText("link");
  expect(written).toEqual(["link"]);
});

test("rejects, not throws, where there is no clipboard (plain http)", async () => {
  withClipboard(undefined);
  const copying = copyText("link");
  await expect(copying).rejects.toBeInstanceOf(TypeError);
});
