import { afterEach, expect, test } from "bun:test";
import { translator } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { paletteSource, type Row } from "./palette-source";

const realFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = realFetch;
});

test("a search's rows replace the previous search's", async () => {
  globalThis.fetch = (async (url: string) => {
    const q = new URL(url, "http://x").searchParams.get("q");
    return Response.json({
      items: [
        {
          id: q === "a" ? 1 : 2,
          title: q,
          price_jpy: 0,
          smart_price: null,
          image: null,
        },
      ],
      itemCount: 1,
      avatars: [],
      authors: [],
      tags: [],
    });
  }) as unknown as typeof fetch;
  const rows = new Map<string, Row>();
  const source = paletteSource({
    t: translator("zh"),
    ui: "zh",
    settings: DEFAULT_SETTINGS,
    rates: {},
    adult: "0",
    rows,
  });
  await source.search("a");
  await source.search("b");
  expect([...rows.keys()].sort()).toEqual(["i:2", "q:b"]);
});
