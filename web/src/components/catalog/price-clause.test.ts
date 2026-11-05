import { describe, expect, test } from "bun:test";
import { priceText } from "./price-clause";

describe("priceText", () => {
  test("both bounds, lower only, upper only, none", () => {
    expect(priceText({ free: false, pmin: 500, pmax: 1500 })).toBe(
      "¥500–1,500",
    );
    expect(priceText({ free: false, pmin: 3000, pmax: 0 })).toBe("¥3,000+");
    expect(priceText({ free: false, pmin: 0, pmax: 500 })).toBe("≤ ¥500");
    expect(priceText({ free: false, pmin: 0, pmax: 0 })).toBe("");
  });
});
