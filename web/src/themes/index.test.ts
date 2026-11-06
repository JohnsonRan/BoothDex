import { expect, test } from "bun:test";
import type { DefinedTheme } from "@astryxdesign/core/theme";
import { themeLoader } from ".";

const matcha = { theme: { name: "matcha" } as DefinedTheme, fonts: [] };

test("a theme loads once and is kept", async () => {
  let calls = 0;
  const loader = themeLoader({
    matcha: async () => {
      calls++;
      return matcha;
    },
  });
  expect(loader.settled("matcha")).toBeUndefined();
  expect(await loader.load("matcha")).toBe(matcha);
  expect(await loader.load("matcha")).toBe(matcha);
  expect(loader.settled("matcha")).toBe(matcha);
  expect(calls).toBe(1);
});

test("a theme that fails to load, or is unknown, settles as null and is not retried", async () => {
  let calls = 0;
  const loader = themeLoader({
    matcha: () => {
      calls++;
      return Promise.reject(new Error("ChunkLoadError"));
    },
  });
  expect(await loader.load("matcha")).toBeNull();
  expect(await loader.load("matcha")).toBeNull();
  expect(calls).toBe(1);
  expect(loader.settled("matcha")).toBeNull();
  expect(await loader.load("charcoal")).toBeNull();
});
