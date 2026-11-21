import { expect, test } from "bun:test";
import { toastClearance } from "@/components/toasts";
import { clearanceAbove } from "./back-to-top";

test("toasts sit above the Back to top button, as far above it as it sits above the window's bottom edge", () => {
  // a 40 px button 16 px above the bottom of a 900 px window: toasts 72 px up, 16 px above the button's top
  expect(clearanceAbove({ top: 844, bottom: 884 }, 900)).toBe(72);
  const seen: number[] = [];
  const stop = toastClearance.subscribe(() => seen.push(toastClearance.get()));
  toastClearance.set(72);
  toastClearance.set(72);
  toastClearance.set(0);
  stop();
  toastClearance.set(30);
  expect(seen).toEqual([72, 0]);
  toastClearance.set(0);
});
