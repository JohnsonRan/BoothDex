import { describe, expect, test } from "bun:test";
import { DEFAULT_FILTERS, filtersToQuery, parseFilters } from "@/lib/catalog";
import { translator } from "@/lib/dictionaries";
import {
  CLAUSE_CLEAR,
  type ClauseKey,
  type ClauseNames,
  clauseParams,
  clauseTexts,
  priceText,
  sortLabel,
  summaryTokens,
  widerSearches,
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

describe("wider searches", () => {
  const f = parseFilters(
    "q=cat&authors=shop&sc=avatar&avatars=manuka&adult=include&free=1&range=7d&if=quest&tags=hair--short&extags=nsfw",
  );
  test("clearing a clause sets defaults the search query leaves out", () => {
    for (const patch of Object.values(CLAUSE_CLEAR))
      expect(filtersToQuery({ ...DEFAULT_FILTERS, ...patch })).toBe("");
  });
  test("each set clause names the parameters that clearing it alone removes", () => {
    const all = clauseTexts(f, names, t);
    const params = clauseParams(f);
    expect(params.map(([key]): string => key)).toEqual(Object.keys(all));
    expect(Object.fromEntries(params)).toEqual({
      author: ["authors"],
      sc: ["sc"],
      avatar: ["avatars"],
      adult: ["adult"],
      price: ["free"],
      range: ["range"],
      features: ["if"],
      tags: ["tags", "extags"],
    });
    for (const [key] of params) {
      const { [key]: _, ...rest } = all;
      expect(clauseTexts({ ...f, ...CLAUSE_CLEAR[key] }, names, t)).toEqual(
        rest,
      );
    }
    expect(clauseParams(parseFilters("pmin=500&pmax=900"))).toEqual([
      ["price", ["pmin", "pmax"]],
    ]);
    expect(clauseParams(parseFilters("q=cat"))).toEqual([]);
  });
  test("counts pair with the clauses; none found is left out, the most first", () => {
    const g = parseFilters("sc=avatar&pmin=500&range=7d&if=quest");
    const w = (key: ClauseKey, label: string, count: number) => ({
      key,
      label,
      count,
    });
    expect(widerSearches(g, names, t, [3, 0, 40, 3])).toEqual([
      w("range", "Published is Past 7 days", 40),
      w("sc", "Categories is Avatars", 3),
      w("features", "Quest", 3),
    ]);
    expect(widerSearches(g, names, t, [])).toEqual([]);
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
