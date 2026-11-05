// Home page (Card Grid template): search, filter tabs and sort above a uniform card grid with a real empty
// state. Everything here renders on the server; the controls it places are client components.

import { Divider } from "@astryxdesign/core/Divider";
import { Layout, LayoutContent, LayoutHeader } from "@astryxdesign/core/Layout";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Heading } from "@astryxdesign/core/Text";
import { Suspense } from "react";
import { Providers } from "@/components/providers";
import { SiteFrame } from "@/components/site-frame";
import { avatarName, type Filters, parseFilters } from "@/lib/catalog";
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
import { AvatarStrip } from "./avatar-strip";
import { FilterUrlProvider } from "./filter-url";
import { Pager } from "./pager";
import { ResultCount } from "./result-count";
import { EmptyResults, FailedResults, SemanticNote } from "./result-states";
import { ResultsGrid, ResultsSkeleton } from "./results-grid";
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
  const t = translator(settings.ui);
  return (
    <Providers
      ui={settings.ui}
      mode={settings.theme}
      catalogue={await componentCatalogue(settings.ui)}
    >
      <SiteFrame label={t("nav.main")}>
        <FilterUrlProvider filters={filters}>
          <Suspense fallback={<ResultsSkeleton />}>
            <Catalog
              settings={settings}
              filters={filters}
              page={page}
              noSemantic={params.sem === "0"}
            />
          </Suspense>
        </FilterUrlProvider>
      </SiteFrame>
    </Providers>
  );
}

async function Catalog({
  settings,
  filters,
  page,
  noSemantic,
}: {
  settings: Settings;
  filters: Filters;
  page: number;
  noSemantic: boolean;
}) {
  const t = translator(settings.ui);
  const [outcome, avatars, categories, rates] = await Promise.all([
    searchItems(filters, page, noSemantic),
    listAvatars().catch(() => []),
    listCategories().catch(() => []),
    settings.currency === "JPY"
      ? ({} as Rates)
      : exchangeRates().catch((): Rates => ({})),
  ]);
  const result = outcome.state === "ok" ? outcome.result : null;
  const avatar = avatars.find((a) => a.slug === filters.avatar);
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
            {result && <ResultCount count={result.count} />}
          </HStack>
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
                categories={categories}
                featureCounts={result?.featureCounts ?? {}}
              />
              <ActiveFilters categories={categories} avatar={avatar} />
            </VStack>
            <Divider />
            {outcome.state !== "ok" ? (
              <FailedResults starting={outcome.state === "starting"} />
            ) : outcome.result.items.length === 0 ? (
              <EmptyResults />
            ) : (
              <VStack gap={6}>
                <ResultsGrid
                  items={outcome.result.items}
                  settings={settings}
                  rates={rates}
                />
                {outcome.result.semantic === "unavailable" && (
                  <SemanticNote text={t("list.semanticUnavailable")} />
                )}
                <Pager
                  page={outcome.result.page}
                  total={outcome.result.totalPages}
                  noSemantic={!!filters.q && outcome.result.semantic !== "used"}
                />
              </VStack>
            )}
          </VStack>
        </LayoutContent>
      }
    />
  );
}
