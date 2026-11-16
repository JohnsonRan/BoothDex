import { expect, test } from "bun:test";
import { ApiError, errorKey } from "@/lib/api";
import { API_ERRORS } from "@/lib/messages";
import { guestbookError } from "./messages";
import { BODY_MAX, bodyOk, NICKNAME_MAX, nicknameOk } from "./rules";

test("the form takes what the server's validate takes", () => {
  expect(nicknameOk("  Ann  ")).toBe(true);
  expect(nicknameOk("   ")).toBe(false);
  expect(nicknameOk("a".repeat(NICKNAME_MAX))).toBe(true);
  expect(nicknameOk("a".repeat(NICKNAME_MAX + 1))).toBe(false);
  // counted in scalar values, as Rust's chars(): an astral character is one
  expect(nicknameOk("😀".repeat(NICKNAME_MAX))).toBe(true);
  expect(nicknameOk("A\tB")).toBe(false);
  expect(bodyOk("line\r\nline\ttab")).toBe(true);
  expect(bodyOk("bell\u0007")).toBe(false);
  expect(bodyOk("lone\rreturn")).toBe(false);
  expect(bodyOk(`${"a".repeat(BODY_MAX)}\n`)).toBe(true);
  expect(bodyOk("a".repeat(BODY_MAX + 1))).toBe(false);
});

test("a closed guestbook's 503 is read by its code, not as the site starting", () => {
  const closed = new ApiError(503, "x", "verification_unavailable");
  expect(errorKey(closed, API_ERRORS, "error.failed")).toBe(
    "list.startingTitle",
  );
  expect(guestbookError(closed)).toBe("closed");
  expect(guestbookError(new ApiError(429, "x"))).toBe("error.tooMany");
  expect(guestbookError(new ApiError(400, "x", "card_number"))).toBe(
    "error.card",
  );
  expect(guestbookError(new Error("offline"))).toBe("error.failed");
});
