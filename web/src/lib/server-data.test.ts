import { expect, test } from "bun:test";
import { visitorHeaders } from "./server-data";

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
