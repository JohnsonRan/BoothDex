import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { ApiError, errorKey } from "@/lib/api";
import { API_ERRORS } from "@/lib/messages";
import { guestbookError } from "./messages";
import {
  BODY_MAX,
  bodyLength,
  bodyOk,
  NICKNAME_MAX,
  nicknameOk,
} from "./rules";

test("the form takes what the server's validate takes", () => {
  expect(nicknameOk("  Ann  ")).toBe(true);
  expect(nicknameOk("   ")).toBe(false);
  expect(nicknameOk("a".repeat(NICKNAME_MAX))).toBe(true);
  expect(nicknameOk("a".repeat(NICKNAME_MAX + 1))).toBe(false);
  // counted in scalar values, as Rust's chars(): an astral character is one
  expect(nicknameOk("😀".repeat(NICKNAME_MAX))).toBe(true);
  expect(nicknameOk("A\tB")).toBe(false);
  expect(nicknameOk("A\u2028B")).toBe(false);
  expect(nicknameOk("A\u2029B")).toBe(false);
  expect(bodyOk("line\r\nline\ttab")).toBe(true);
  expect(bodyOk("bell\u0007")).toBe(false);
  expect(bodyOk("lone\rreturn")).toBe(false);
  expect(bodyOk(`${"a".repeat(BODY_MAX)}\n`)).toBe(true);
  expect(bodyOk("a".repeat(BODY_MAX + 1))).toBe(false);
  // the counter counts what the server counts: scalar values of the trimmed text, so a heart with its variation
  // selector is two (one character to a reader), as Rust's chars() and PostgreSQL's char_length count it
  expect(bodyLength(" \u2764\ufe0f\r\n")).toBe(2);
  expect(bodyLength("\u2764\ufe0f".repeat(BODY_MAX / 2))).toBe(BODY_MAX);
  // and an astral character is one, not its two UTF-16 units
  expect(bodyLength("\u{1f600}")).toBe(1);
  expect(bodyOk("\u2764\ufe0f".repeat(BODY_MAX / 2 + 1))).toBe(false);
  // trimmed as Rust trims: the BOM stays and is refused, U+0085 goes
  expect(nicknameOk("\ufeffAnn")).toBe(false);
  expect(nicknameOk("Ann\u0085")).toBe(true);
});

/** The characters of a `const NAME: &[(char, char)]` table in the server's guestbook.rs. */
function serverList(name: string) {
  const source = readFileSync(
    new URL("../../../../crates/api/src/guestbook.rs", import.meta.url),
    "utf8",
  );
  const table = source.split(`const ${name}: &[(char, char)] = &[`)[1];
  const found: string[] = [];
  for (const [, from, to] of table
    .slice(0, table.indexOf("];"))
    .matchAll(/'\\u\{(\w+)\}', '\\u\{(\w+)\}'/g))
    for (let c = Number.parseInt(from, 16); c <= Number.parseInt(to, 16); c++)
      found.push(String.fromCodePoint(c));
  return found;
}

test("the form refuses every invisible character the server refuses", () => {
  const invisible = serverList("INVISIBLE");
  expect(invisible.length).toBeGreaterThan(40);
  for (const c of invisible) {
    expect(nicknameOk(`A${c}nn`)).toBe(false);
    expect(bodyOk(`h${c}i`)).toBe(false);
  }
});

test("what shapes the text beside it is kept, but not on its own", () => {
  expect(nicknameOk("👨\u200d👩\u200d👧")).toBe(true);
  expect(bodyOk("❤\ufe0f")).toBe(true);
  const shaping = serverList("SHAPING");
  expect(shaping.length).toBeGreaterThan(300);
  for (const c of shaping) {
    expect(nicknameOk(`A${c}nn`)).toBe(true);
    expect(nicknameOk(`${c} ${c}`)).toBe(false);
    expect(bodyOk(`${c} ${c}`)).toBe(false);
  }
});

test("a closed guestbook's 503 is read by its code, not as the site starting", () => {
  const closed = new ApiError(503, "x", "verification_unavailable");
  expect(errorKey(closed, API_ERRORS, "error.failed")).toBe(
    "list.startingTitle",
  );
  expect(guestbookError(closed)).toBe("closed");
  expect(guestbookError(new ApiError(429, "x"))).toBe("error.tooMany");
  expect(guestbookError(new Error("offline"))).toBe("error.failed");
});
