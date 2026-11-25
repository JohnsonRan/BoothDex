import { expect, test } from "bun:test";
import { retryAction } from "./check-dialog";

test("Try again reloads a failed widget and resets an empty or expired one", () => {
  expect(retryAction({ token: "", failed: true })).toBe("retry");
  expect(retryAction({ token: "", failed: false })).toBe("reset");
  expect(retryAction({ token: "ready", failed: false })).toBe("none");
});
