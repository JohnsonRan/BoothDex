// Home page (Card Grid template): search, filter tabs and sort above a uniform card grid with a real empty
// state. Everything here renders on the server; the controls it places are client components. The frame
// (title, controls) streams first; the count and the results follow when the search answers, Skeleton
// silhouettes holding their places (loading.tsx), and enter once if the visitor saw those (arrival.tsx).

import { Divider } from "@astryxdesign/core/Divider";
import { Layout, LayoutContent, LayoutHeader } from "@astryxdesign/core/Layout";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Heading } from "@astryxdesign/core/Text";
import { Suspense } from "react";
import { SiteFrame } from "@/components/frame/site-frame";
import { Providers } from "@/components/providers";
import {
  avatarName,
  type Filters,
  localName,
  parseFilters,
} from "@/lib/catalog";
import {
  componentCatalogue,
  exchangeRates,
  listAvatars,
  listCategories,
  readSettings,
  searchItems,
} from "@/lib/server-data";
import { type Rates, type Settings, translator } from "@/lib/settings-core";
import { ActiveFilters } from "./active-filters";
import { Arrival } from "./arrival";
import { AvatarStrip } from "./avatar-strip";
import { FilterUrlProvider } from "./filter-url";
import { CountSilhouette, ResultsSilhouette } from "./loading";
import { RefreshProgress } from "./refresh";
import { ResultCount, Results } from "./results";
import { SearchBox } from "./search-box";
import { Toolbar } from "./toolbar";

type Params = Record<string, string | string[] | undefined>;

/** The search part of the URL; `page`, `sem` and `item` are read separately. */
function filtersFrom(params: Params): Filters {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(params))
    for (const one of [v ?? []].flat()) p.append(k, one);
  for (const k of ["page", "sem", "item"]) p.delete(k);
  return parseFilters(p.toString());
}

export async function HomeScreen({
  searchParams,
}: {
  searchParams: Promise<Params>;
}) {
  const [settings, params] = await Promise.all([readSettings(), searchParams]);
  const filters = filtersFrom(params);
  const page = Math.max(1, Number.parseInt(String(params.page), 10) || 1);
  // started here: the palette (item prices) and the results use them; nothing waits for them
  const rates =
    settings.currency === "JPY"
      ? Promise.resolve({} as Rates)
      : exchangeRates().catch((): Rates => ({}));
  return (
    <Providers
      settings={settings}
      catalogue={await componentCatalogue(settings.ui)}
    >
      <FilterUrlProvider filters={filters}>
        <SiteFrame rates={rates}>
          {/* no fallback: the frame waits only for the cached avatar and category lists */}
          <Arrival>
            <Catalog
              settings={settings}
              filters={filters}
              page={page}
              noSemantic={params.sem === "0"}
              rates={rates}
            />
          </Arrival>
        </SiteFrame>
      </FilterUrlProvider>
    </Providers>
  );
}

async function Catalog({
  settings,
  filters,
  page,
  noSemantic,
  rates,
}: {
  settings: Settings;
  filters: Filters;
  page: number;
  noSemantic: boolean;
  rates: Promise<Rates>;
}) {
  const t = translator(settings.ui);
  // started here, awaited by the count and the results: the frame does not wait for it
  const outcome = searchItems(filters, page, noSemantic);
  const featureCounts = outcome.then((o) =>
    o.state === "ok" ? o.result.featureCounts : {},
  );
  const [avatars, categories] = await Promise.all([
    listAvatars().catch(() => []),
    listCategories().catch(() => []),
  ]);
  const avatar = avatars.find((a) => a.slug === filters.avatar);
  // only names cross to the browser, not the whole taxonomy in every language
  const categoryTabs = categories
    .filter((c) => c.mode === "items")
    .map((c): [string, string] => [c.slug, localName(c, settings.ui)]);
  const category = categories
    .flatMap((c) => [c, ...c.children])
    .find((c) => c.slug === filters.sc);
  const title = filters.q
    ? t("title.search", { q: filters.q })
    : filters.author
      ? t("title.author", { name: filters.author })
      : avatar
        ? t("title.avatar", { name: avatarName(avatar, settings.ui) })
        : filters.view === "avatars"
          ? t("title.avatars")
          : filters.view === "favourites"
            ? t("title.favourites")
            : t("title.all");

  return (
    <Layout
      height="auto"
      padding={6}
      header={
        <LayoutHeader hasDivider>
          <HStack gap={3} vAlign="center" wrap="wrap">
            <Heading level={1}>{title}</Heading>
            <Suspense fallback={<CountSilhouette />}>
              <ResultCount outcome={outcome} settings={settings} />
            </Suspense>
          </HStack>
          <RefreshProgress />
        </LayoutHeader>
      }
      content={
        <LayoutContent isScrollable={false}>
          <VStack gap={6}>
            <VStack gap={4}>
              <SearchBox />
              {filters.view === "items" && (
                <AvatarStrip avatars={avatars} value={filters.avatar} />
              )}
              <Toolbar
                categoryTabs={categoryTabs}
                featureCounts={featureCounts}
              />
              <ActiveFilters
                categoryName={category && localName(category, settings.ui)}
                avatar={avatar}
              />
            </VStack>
            <Divider />
            <Suspense
              fallback={<ResultsSilhouette label={t("list.loading")} />}
            >
              <Results
                outcome={outcome}
                rates={rates}
                settings={settings}
                filters={filters}
              />
            </Suspense>
          </VStack>
        </LayoutContent>
      }
    />
  );
}
