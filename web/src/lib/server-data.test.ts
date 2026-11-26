import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DEFAULT_FILTERS } from "./catalog";
import {
  isPhoneHeaders,
  passOffer,
  relatedSearches,
  type SearchOutcome,
  searchFailure,
  tagLabels,
  visitorHeaders,
} from "./server-data";

test("visitorHeaders forwards the session and the visitor's address headers, and nothing else", () => {
  const from = new Headers({
    cookie: "session=abc",
    "x-forwarded-for": "203.0.113.7, 10.0.0.2",
    "x-real-ip": "203.0.113.7",
    "cf-connecting-ip": "203.0.113.7",
    "accept-language": "ja",
    host: "boothdex.example",
  });
  expect(visitorHeaders(from)).toEqual({
    cookie: "session=abc",
    "x-forwarded-for": "203.0.113.7, 10.0.0.2",
    "x-real-ip": "203.0.113.7",
    "cf-connecting-ip": "203.0.113.7",
  });
});

test("visitorHeaders leaves out the headers a request did not carry", () => {
  expect(visitorHeaders(new Headers({ "x-real-ip": "198.51.100.4" }))).toEqual({
    "x-real-ip": "198.51.100.4",
  });
  expect(visitorHeaders(new Headers())).toEqual({});
});

test("a read refused by the stricter limit carries the check it offers; other failures carry none", async () => {
  const failed = (status: number, body: unknown) =>
    new Error(`/api/search: ${status}`, {
      cause: Response.json(body, { status }),
    });
  const offered = {
    ok: false,
    code: "verification_offered",
    error: "Too many requests. Try again shortly.",
    site_key: "0x-site",
  };
  expect(await passOffer(failed(429, offered))).toEqual({ siteKey: "0x-site" });
  expect(
    await passOffer(failed(429, { ok: false, error: "Too many requests." })),
  ).toBeUndefined();
  expect(await passOffer(failed(500, offered))).toBeUndefined();
  expect(await passOffer(new TypeError("fetch failed"))).toBeUndefined();
});

test("isPhoneHeaders detects Chromium mobile hints and phone user agents", () => {
  expect(isPhoneHeaders(new Headers({ "sec-ch-ua-mobile": "?1" }))).toBe(true);
  expect(
    isPhoneHeaders(
      new Headers({
        "user-agent":
          "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15",
      }),
    ),
  ).toBe(true);
  expect(
    isPhoneHeaders(
      new Headers({
        "user-agent":
          "Mozilla/5.0 (Linux; Android 14; Pixel 8) Mobile Safari/537.36",
      }),
    ),
  ).toBe(true);
  expect(
    isPhoneHeaders(
      new Headers({
        "sec-ch-ua-mobile": "?0",
        "user-agent":
          "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0",
      }),
    ),
  ).toBe(false);
  expect(isPhoneHeaders(new Headers())).toBe(false);
});

test("searchFailure maps 401 to unauthorized, 429 to limited error, and network failures to starting or error", async () => {
  const err = (status: number, body: unknown = {}) =>
    new Error(`/api/search: ${status}`, {
      cause: Response.json(body, { status }),
    });
  expect(await searchFailure(err(401), async () => true)).toEqual({
    state: "unauthorized",
  });
  expect(
    await searchFailure(
      err(429, {
        ok: false,
        code: "verification_offered",
        site_key: "0x-site",
      }),
      async () => true,
    ),
  ).toEqual({
    state: "error",
    limited: true,
    offer: { siteKey: "0x-site" },
  });
  expect(await searchFailure(err(500), async () => true)).toEqual({
    state: "error",
  });
  expect(
    await searchFailure(new TypeError("fetch failed"), async () => false),
  ).toEqual({ state: "starting" });
});

test("a visitor whose search a rate limit refused gets the tags' names, and no lookup is made", async () => {
  const asked: string[] = [];
  const lookup = async (name: string) => {
    asked.push(name);
    return { name, display: name.toUpperCase() };
  };
  const labels = (o: SearchOutcome) =>
    tagLabels(["long_hair", "cat_ears"], "en", Promise.resolve(o), lookup);
  for (const refused of [
    { state: "error", limited: true, offer: { siteKey: "0x-site" } },
    { state: "error", limited: true },
  ] as const)
    expect(await labels(refused)).toEqual({
      long_hair: "long_hair",
      cat_ears: "cat_ears",
    });
  expect(asked).toEqual([]);
  // any other failure still reads the labels
  expect(await labels({ state: "error" })).toEqual({
    long_hair: "LONG_HAIR",
    cat_ears: "CAT_EARS",
  });
  expect(asked).toEqual(["long_hair", "cat_ears"]);
});

test("a text search's tag labels are read at once, without waiting for the search", async () => {
  const asked: string[] = [];
  const lookup = async (name: string) => {
    asked.push(name);
    return { name, display: name.toUpperCase() };
  };
  // no search handed over, as for a text search: nothing to wait for, even if that search is refused later
  expect(await tagLabels(["long_hair"], "en", undefined, lookup)).toEqual({
    long_hair: "LONG_HAIR",
  });
  expect(asked).toEqual(["long_hair"]);
});

test("related searches are asked once a search matched by meaning or found nothing; a refusal or a failure gives none", async () => {
  const asked: { url: string; headers: unknown }[] = [];
  let answer: () => Response = () =>
    Response.json({ searches: [{ label: "long skirt", q: "long skirt" }] });
  const real = globalThis.fetch;
  globalThis.fetch = (async (url: string, init?: RequestInit) => {
    asked.push({ url: String(url), headers: init?.headers });
    return answer();
  }) as typeof fetch;
  try {
    const filters = { ...DEFAULT_FILTERS, q: "skirt", free: true };
    const found = (semantic: string, count = 1) =>
      relatedSearches(
        Promise.resolve({
          state: "ok",
          ms: 1,
          result: { semantic, count },
        } as unknown as SearchOutcome),
        filters,
        { "x-real-ip": "203.0.113.7" },
        "ja",
      );
    expect(await found("used")).toEqual([
      { label: "long skirt", q: "long skirt" },
    ]);
    // the visitor's language labels the searches
    expect(asked).toEqual([
      {
        url: expect.stringMatching(/\/api\/related\?q=skirt&free=1&hl=ja$/),
        headers: { "x-real-ip": "203.0.113.7" },
      },
    ]);
    // found items without meaning-based matches, so no embedding stored by this search; or no query or provider
    for (const [semantic, count] of [
      ["skipped", 1],
      ["off", 1],
      ["unavailable", 1],
      ["pending", 1],
      ["pending", 0],
      ["off", 0],
    ] as const)
      expect(await found(semantic, count)).toEqual([]);
    expect(
      await relatedSearches(
        Promise.resolve({ state: "error", limited: true }),
        filters,
        {},
        "en",
      ),
    ).toEqual([]);
    expect(asked.length).toBe(1);
    // nothing found: asked, from this search's embedding or one an earlier search stored
    for (const semantic of ["used", "skipped", "unavailable"])
      expect(await found(semantic, 0)).toEqual([
        { label: "long skirt", q: "long skirt" },
      ]);
    expect(asked.length).toBe(4);
    // a rate limit, even one offering the check, an API without the endpoint, or a failed request: none
    for (const refusal of [
      () =>
        Response.json(
          { ok: false, code: "verification_offered", site_key: "0x-site" },
          { status: 429 },
        ),
      () => new Response("", { status: 404 }),
      () => {
        throw new TypeError("fetch failed");
      },
    ]) {
      answer = refusal;
      expect(await found("used")).toEqual([]);
    }
  } finally {
    globalThis.fetch = real;
  }
});

test("the home page asks for related searches in the visitor's interface language", () => {
  // an async server component that reads the request: the call is checked as written
  const home = readFileSync(
    join(import.meta.dir, "../components/catalog/home-screen.tsx"),
    "utf8",
  );
  const args = home.match(/= relatedSearches\(([\s\S]*?)\);/)?.[1] ?? "";
  expect(
    args
      .split(",")
      .map((a) => a.trim())
      .filter(Boolean)
      .at(-1),
  ).toBe("settings.ui");
});

test("related searches that do not answer in time give none, so the results do not wait for them", async () => {
  const real = globalThis.fetch;
  // an API that never answers, such as one waiting for a search permit, until the request is given up
  globalThis.fetch = ((_url: string, init?: RequestInit) =>
    new Promise((_, reject) =>
      init?.signal?.addEventListener("abort", () =>
        reject(init.signal?.reason),
      ),
    )) as typeof fetch;
  try {
    const searched = Promise.resolve({
      state: "ok",
      ms: 1,
      result: { semantic: "used", count: 1 },
    } as unknown as SearchOutcome);
    expect(
      await relatedSearches(
        searched,
        { ...DEFAULT_FILTERS, q: "skirt" },
        {},
        "zh",
      ),
    ).toEqual([]);
  } finally {
    globalThis.fetch = real;
  }
});
