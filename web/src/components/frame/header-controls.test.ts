import { expect, test } from "bun:test";
import { toggledTheme } from "./header-controls";

test("ColorModeToggle reverts to system when toggled back to the OS appearance", () => {
  expect(toggledTheme("system", false)).toBe("dark");
  expect(toggledTheme("dark", false)).toBe("system");
  expect(toggledTheme("system", true)).toBe("light");
  expect(toggledTheme("light", true)).toBe("system");
  expect(toggledTheme("dark", true)).toBe("light");
  expect(toggledTheme("light", false)).toBe("dark");
});
