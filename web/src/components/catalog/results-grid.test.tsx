import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { ItemResponse } from "@/lib/api-types";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { FIRST_TILES, ResultsGrid } from "./results-grid";

const item: ItemResponse = {
  id: 1,
  title: "Test item",
  bio: "",
  author_name: "Test shop",
  author_uri: "",
  author_icon: null,
  booth_category: null,
  price_jpy: 1000,
  smart_price: null,
  smart_price_reason: null,
  likes: 0,
  is_adult: false,
  content_rating: "SFW",
  quest: null,
  features: [],
  status: "ok",
  published_at: null,
  scraped_at: "2026-01-01T00:00:00Z",
  media: [],
  booth_tags: [],
  booth_images: ["https://booth.pximg.net/u/i/1/cover.jpg"],
  booth_images_complete: true,
  relisted_as: null,
  variations: [],
  translations: { en: { title: "English title" } },
  linked_avatars: [],
  categories: [],
  original_language: null,
  avatar_slug: null,
  supported: false,
  gender: null,
  needs_enrichment: false,
  last_listed_at: null,
  enrich_failures: 0,
};

const render = (items: ItemResponse[], content = DEFAULT_SETTINGS.content) =>
  renderToStaticMarkup(
    <ResultsGrid
      items={items}
      settings={{ ...DEFAULT_SETTINGS, content }}
      rates={{}}
    />,
  );

test("the server HTML carries every cover, the first ones loading at once", () => {
  const items = Array.from({ length: 20 }, (_, i) => ({ ...item, id: i + 1 }));
  const html = render(items);
  expect(html.match(/<img /g)).toHaveLength(20);
  expect(html).toContain('src="/img/b/300x300_a2_g5/u/i/1/cover.jpg"');
  expect(html.match(/<img [^>]*fetchPriority="high"/gi)).toHaveLength(4);
  expect(html.match(/loading="eager"/g)).toHaveLength(12);
});

test("a full page renders the first screen on the server and the rest in the browser", () => {
  const items = Array.from({ length: 100 }, (_, i) => ({ ...item, id: i + 1 }));
  const html = render(items);
  expect(html.match(/<img /g)).toHaveLength(FIRST_TILES);
  // one placeholder cell after the last server tile; the other tiles are drawn near the viewport
  expect(html.match(/class="astryx-skeleton/g)).toHaveLength(1);
});

test("tiles show the title in the chosen language, the shop and the price", () => {
  const html = render([item], "en");
  expect(html).toContain("English title");
  expect(html).toContain("Test shop");
  expect(html).toContain("¥1,000");
});

test("new items and feature tags are marked; plain items get no badges", () => {
  const html = render([
    {
      ...item,
      id: 1,
      published_at: new Date().toISOString(),
      features: ["quest", "ma", "liltoon"],
    },
    { ...item, id: 2 },
  ]);
  expect(html).toContain("NEW");
  expect(html).toContain("Quest");
  expect(html).toContain("MA");
  expect(html).not.toContain("lilToon");
  expect(html.match(/class="astryx-badge /g)).toHaveLength(3);
});
