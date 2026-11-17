import { expect, test } from "bun:test";
import { passOffer, visitorHeaders } from "./server-data";

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
