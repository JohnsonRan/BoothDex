import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { Item } from "@/lib/catalog";
import { SettingsProvider } from "@/lib/settings";
import { ItemCard } from "./item-card";

const item: Item = {
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
  booth_images: [
    "https://booth.pximg.net/u/i/1/cover.jpg",
    "https://booth.pximg.net/u/i/1/preview.jpg",
  ],
  booth_images_complete: true,
  relisted_as: null,
  variations: [],
  translations: {},
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

function renderCard(booth_images: string[]) {
  return renderToStaticMarkup(
    <SettingsProvider>
      <ItemCard
        item={{ ...item, booth_images }}
        favourite={false}
        viewed={false}
        onOpen={() => {}}
        onFavourite={() => {}}
      />
    </SettingsProvider>,
  );
}

test("cards expose their cover and title without requesting hover previews", () => {
  const html = renderCard(item.booth_images ?? []);
  expect(html).toContain('src="/img/b/300x300_a2_g5/u/i/1/cover.jpg"');
  expect(html).not.toContain("preview.jpg");
  // the open button is named by its visible text, not an aria-label that could drift from it
  expect(html).toMatch(/<button type="button" class="block w-full text-left">/);
  expect(html).toContain(">Test item</h2>");
  expect(html.match(/<img /g)).toHaveLength(1);
});

test("single-image cards still render their cover", () => {
  expect(renderCard(item.booth_images?.slice(0, 1) ?? [])).toContain(
    'src="/img/b/300x300_a2_g5/u/i/1/cover.jpg"',
  );
});
