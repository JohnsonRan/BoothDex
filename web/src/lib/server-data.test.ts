import { expect, test } from "bun:test";
import {
  passOffer,
  type SearchOutcome,
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
