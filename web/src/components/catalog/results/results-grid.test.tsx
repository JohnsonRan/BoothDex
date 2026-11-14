import { expect, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import { renderToStaticMarkup } from "react-dom/server";
import { AccountProvider } from "@/components/account/session";
import { Providers } from "@/components/providers";
import type { ItemResponse } from "@/lib/api-types";
import { MESSAGES, translator } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { FIRST_TILES } from "./item-tile";
import { ResultsGrid } from "./results-grid";

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

// the viewed marks read the session, and the settings need a router to apply changes
const render = (items: ItemResponse[], content = DEFAULT_SETTINGS.content) =>
  renderToStaticMarkup(
    <AppRouterContext value={{} as AppRouterInstance}>
      <Providers
        settings={DEFAULT_SETTINGS}
        messages={MESSAGES[DEFAULT_SETTINGS.ui]}
      >
        <AccountProvider>
          <ResultsGrid
            items={items}
            settings={{ ...DEFAULT_SETTINGS, content }}
            rates={{}}
            t={translator(DEFAULT_SETTINGS.ui)}
          />
        </AccountProvider>
      </Providers>
    </AppRouterContext>,
  );

test("the server HTML carries the first screen's covers, the first ones loading at once", () => {
  const items = Array.from({ length: FIRST_TILES }, (_, i) => ({
    ...item,
    id: i + 1,
  }));
  const html = render(items);
  expect(html.match(/<img /g)).toHaveLength(FIRST_TILES);
  expect(html).toContain(
    'src="/img/b/300x300_a2_g5/u/i/1/cover.jpg?format=avif"',
  );
  expect(html.match(/<img [^>]*fetchPriority="high"/gi)).toHaveLength(6);
  expect(html.match(/loading="eager"/g)).toHaveLength(18);
});

test("a full page renders the first screen on the server and the rest in the browser", () => {
  const items = Array.from({ length: 100 }, (_, i) => ({ ...item, id: i + 1 }));
  const html = render(items);
  expect(html.match(/<img /g)).toHaveLength(FIRST_TILES);
  // a Skeleton under every server cover, and one tile silhouette after the last server tile (cover, two title
  // lines, shop icon and name, price); the other tiles are drawn near the viewport
  expect(html.match(/class="astryx-skeleton/g)).toHaveLength(FIRST_TILES + 6);
});

test("every tile reserves two title lines, as its loading silhouette does", () => {
  const html = render([item, { ...item, id: 2 }]);
  expect(html.match(/min-block-size:2lh/g)).toHaveLength(2);
});

test("a tile's badges sit on its cover, and its second image waits for a pointer", () => {
  const html = render([
    {
      ...item,
      features: ["quest"],
      booth_images: [
        ...item.booth_images,
        "https://booth.pximg.net/u/i/1/second.jpg",
      ],
    },
  ]);
  const cover = html.slice(html.indexOf('class="tile-cover"'));
  expect(cover.indexOf("Quest")).toBeGreaterThan(-1);
  expect(cover.indexOf("Quest")).toBeLessThan(cover.indexOf("<h4"));
  const second = html.match(/<img [^>]*class="tile-second"[^>]*>/)?.[0] ?? "";
  expect(second).toContain(
    'data-second="/img/b/300x300_a2_g5/u/i/1/second.jpg?format=avif"',
  );
  expect(second).not.toMatch(/ src=/);
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

test("before the session is read, favourite buttons are unpressed and no cover is dimmed", () => {
  const html = render([item, { ...item, id: 2 }]);
  expect(html.match(/aria-pressed="false"/g)).toHaveLength(2);
  expect(html).toContain(`data-fav="2"`);
  expect(html).not.toContain("astryx-overlay");
});
