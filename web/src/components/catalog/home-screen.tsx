// Home page: the table-filter template's header (page.tsx:4339-4475: progress bar, title row, toolbar row of
// filter row and end cluster) over the avatar strip and a uniform card grid with a real empty state. Everything
// here renders on the server; the controls it places are client components. The frame (title, controls) streams
// first; the count and the results follow when the search answers, Skeleton silhouettes holding their places
// (loading.tsx), and enter once if the visitor saw those (arrival.tsx).

import { Layout, LayoutContent, LayoutHeader } from "@astryxdesign/core/Layout";
import { Section } from "@astryxdesign/core/Section";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Heading } from "@astryxdesign/core/Text";
import { Suspense } from "react";
import { RecentCount, RecentResults } from "@/components/account/recent-view";
import { ViewActions } from "@/components/account/view-actions";
import { SiteFrame } from "@/components/frame/site-frame";
import { Providers } from "@/components/providers";
import {
  avatarName,
  type Filters,
  localName,
  parseFilters,
} from "@/lib/catalog";
import { MESSAGES, translator } from "@/lib/dictionaries";
import {
  componentCatalogue,
  exchangeRates,
  listAvatars,
  listCategories,
  readSettings,
  type SearchOutcome,
  searchItems,
  tagLabels,
} from "@/lib/server-data";
import type { Rates, Settings } from "@/lib/settings-core";
import { Arrival } from "./arrival";
import { AvatarStrip } from "./avatar-strip";
import type { ClauseNames } from "./clause-text";
import type { Facets } from "./filter-style";
import { FilterUrlProvider } from "./filter-url";
import { CountSilhouette, ResultsSilhouette } from "./loading";
import { RefreshProgress } from "./refresh";
import { ResultCount, Results } from "./results";
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
  // started here: the item dialog (prices) and the results use them; nothing waits for them
  const rates =
    settings.currency === "JPY"
      ? Promise.resolve({} as Rates)
      : exchangeRates().catch((): Rates => ({}));
  return (
    <Providers
      settings={settings}
      messages={MESSAGES[settings.ui]}
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
  const recent = filters.view === "recent";
  // nothing is searched for the recent view: the browser reads it (account/recent.tsx)
  const outcome: Promise<SearchOutcome> = recent
    ? Promise.resolve({ state: "error" })
    : searchItems(filters, page, noSemantic);
  const [avatars, categories, tags] = await Promise.all([
    listAvatars().catch(() => []),
    listCategories().catch(() => []),
    tagLabels([...filters.tags, ...filters.extags], settings.ui),
  ]);
  const avatar = avatars.find((a) => a.slug === filters.avatar);
  // only names cross to the browser, not the whole taxonomy in every language
  const names: ClauseNames = {
    categories: categories
      .filter((c) => c.mode === "items")
      .map((c) => [c.slug, localName(c, settings.ui)]),
    // one name per avatar, the visitor's (avatarName falls back to English)
    avatars: avatars.map((a) => [a.slug, avatarName(a, settings.ui)]),
    tags,
  };
  // the category clause shows only its own counts: the subcategories' are left on the server
  const tabSlugs = new Set(names.categories.map(([slug]) => slug));
  const facets = outcome.then(
    (o): Facets =>
      o.state === "ok"
        ? {
            count: o.result.count,
            smart: Object.fromEntries(
              Object.entries(o.result.smartCounts).filter(([k]) =>
                tabSlugs.has(k),
              ),
            ),
            features: o.result.featureCounts,
          }
        : { count: 0, smart: {}, features: {} },
  );
  const title = recent
    ? t("title.recent")
    : filters.q
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
        // padding 0 and a Section inside, as the template (:4339-4353), so the progress bar runs along the header's
        // top edge; the Section keeps the page inset (TEMPLATE-MAP rule 5) where the template has 16 px
        <LayoutHeader hasDivider padding={0} label={t("filter.region")}>
          <VStack gap={0} style={{ position: "relative" }}>
            <RefreshProgress />
            <Section
              variant="transparent"
              padding={6}
              className="filter-toolbar"
            >
              <VStack gap={4}>
                <HStack gap={3} vAlign="center" wrap="wrap">
                  <Heading level={1}>{title}</Heading>
                  {recent && <RecentCount />}
                  {(recent || filters.view === "favourites") && <ViewActions />}
                </HStack>
                {/* filters and sort do not apply to the recent view; the row is its own boundary, so a click on
                    it before the page has hydrated hydrates the row at once, not the rest with it */}
                {!recent && (
                  <Suspense>
                    <Toolbar
                      title={title}
                      facets={facets}
                      names={names}
                      count={
                        <Suspense fallback={<CountSilhouette />}>
                          <ResultCount
                            outcome={outcome}
                            settings={settings}
                            hasQuery={!!filters.q}
                          />
                        </Suspense>
                      }
                    />
                  </Suspense>
                )}
              </VStack>
            </Section>
          </VStack>
        </LayoutHeader>
      }
      content={
        <LayoutContent isScrollable={false}>
          <VStack gap={6}>
            {filters.view === "items" && (
              <Suspense>
                <AvatarStrip
                  avatars={avatars.map((a) => ({
                    slug: a.slug,
                    en: avatarName(a, settings.ui),
                    image_url: a.image_url,
                  }))}
                />
              </Suspense>
            )}
            <Suspense
              fallback={<ResultsSilhouette label={t("list.loading")} />}
            >
              {recent ? (
                <RecentResults settings={settings} rates={rates} />
              ) : (
                <Results
                  outcome={outcome}
                  rates={rates}
                  settings={settings}
                  filters={filters}
                  page={page}
                  noSemantic={noSemantic}
                />
              )}
            </Suspense>
          </VStack>
        </LayoutContent>
      }
    />
  );
}
