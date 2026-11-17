import { expect, test } from "bun:test";
import { ApiError } from "@/lib/api";
import { verificationRefused } from "./messages";

test("only a refused token reads as a failed verification", () => {
  expect(
    verificationRefused(new ApiError(403, "x", "verification_failed")),
  ).toBe(true);
  expect(verificationRefused(new ApiError(403, "x", "admins_only"))).toBe(
    false,
  );
  expect(verificationRefused(new Error("offline"))).toBe(false);
});
