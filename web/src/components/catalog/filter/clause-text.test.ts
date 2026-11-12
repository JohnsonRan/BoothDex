import { describe, expect, test } from "bun:test";
import { DEFAULT_FILTERS, parseFilters } from "@/lib/catalog";
import { translator } from "@/lib/dictionaries";
import {
  type ClauseNames,
  clauseTexts,
  priceText,
  sortLabel,
  summaryTokens,
} from "./clause-text";

describe("priceText", () => {
  test("both bounds, lower only, upper only, none", () => {
    expect(priceText({ pmin: 500, pmax: 1500 })).toBe("¥500–1,500");
    expect(priceText({ pmin: 3000, pmax: 0 })).toBe("¥3,000+");
    expect(priceText({ pmin: 0, pmax: 500 })).toBe("≤ ¥500");
    expect(priceText({ pmin: 0, pmax: 0 })).toBe("");
  });
});

const names: ClauseNames = {
  categories: [["avatar", "Avatars"]],
  avatars: [["manuka", "マヌカ"]],
  tags: { "hair--short": "Short hair" },
};
const t = translator("en");

describe("clauseTexts", () => {
  test("nothing set reads as no clauses", () => {
    expect(clauseTexts(DEFAULT_FILTERS, names, t)).toEqual({});
  });
  test("set clauses read as the template's triggers, with display names", () => {
    const f = parseFilters(
      "sc=avatar&avatars=manuka&adult=include&pmin=500&range=7d&if=quest+ma&tags=hair--short&extags=nsfw",
    );
    expect(clauseTexts(f, names, t)).toEqual({
      sc: "Categories is Avatars",
      avatar: "Avatar is マヌカ",
      adult: "Content rating is Everything",
      price: "Price ≥ ¥500",
      range: "Published is Past 7 days",
      features: "Quest, +1",
      tags: "Short hair, +1",
    });
  });
  test("the avatar view has gender instead of category", () => {
    const f = parseFilters("v=avatars&gender=female&sc=avatar");
    expect(clauseTexts(f, names, t)).toEqual({ gender: "Gender is Female" });
  });
});

describe("summaryTokens", () => {
  test("query, presets and clauses in row order", () => {
    const f = parseFilters("v=avatars&q=cat&free=1");
    expect(summaryTokens(f, names, t)).toEqual([
      "Search cat",
      "Avatar bodies",
      "Known avatars only",
      "Price is Free",
    ]);
  });
  test("multi-value clauses name their field; an upper bound reads as its comparison", () => {
    const f = parseFilters("pmax=500&if=quest&tags=hair--short");
    expect(summaryTokens(f, names, t)).toEqual([
      "Price ≤ ¥500",
      "Features include Quest",
      "Tags include Short hair",
    ]);
  });
});

describe("sortLabel", () => {
  test("a text search's default order is relevance", () => {
    expect(sortLabel(parseFilters("q=cat"), t)).toBe("Relevance");
    expect(sortLabel(parseFilters("sort=price&dir=asc"), t)).toBe(
      "Price: low to high",
    );
  });
});
