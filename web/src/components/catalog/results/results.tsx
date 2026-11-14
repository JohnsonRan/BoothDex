// The parts of the home page that wait for the search: the result count in the filter row's readout (after the page
// in view, from page 2 on), with the readout's "Clear all" so the two arrive together, and the results region. Both
// read the same pending search. They come with the frame; for a text search each is behind its own Suspense
// boundary (home-screen.tsx, Pending), so the frame around them is painted first.

import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { VisuallyHidden } from "@astryxdesign/core/VisuallyHidden";
import {
  type ClauseNames,
  widerSearches,
} from "@/components/catalog/filter/clause-text";
import { ClearAll } from "@/components/catalog/filter/filter-bar";
import { KeepResults, Swap } from "@/components/catalog/refresh";
import { SearchTrace } from "@/components/catalog/search/search-trace";
import { type Filters, filtersToQuery } from "@/lib/catalog";
import { translator } from "@/lib/dictionaries";
import { drawnWith, pageQuery } from "@/lib/result-pages";
import type { SearchOutcome } from "@/lib/server-data";
import { LOCALES, type Rates, type Settings } from "@/lib/settings-core";
import { FIRST_TILES, tileOf } from "./item-tile";
import { PageInView } from "./page-in-view";
import { ResultPages } from "./result-pages";
import { EmptyResults, SemanticNote } from "./result-states";

export async function ResultCount({
  outcome,
  settings,
  hasQuery,
}: {
  outcome: Promise<SearchOutcome>;
  settings: Settings;
  /** a text search: the count opens how it was answered (search-trace.tsx) */
  hasQuery: boolean;
}) {
  const o = await outcome;
  if (o.state !== "ok") return <ClearAll afterCount={false} />;
  const t = translator(settings.ui);
  const count = t("title.count", {
    n: o.result.count.toLocaleString(LOCALES[settings.ui]),
  });
  return (
    <Swap>
      <HStack gap={2} vAlign="center" className="readout-line">
        <PageInView page={o.result.page} />
        {hasQuery ? (
          <SearchTrace
            count={count}
            semantic={o.result.semantic}
            steps={o.result.trace}
            serverMs={o.result.timing.totalMs}
            totalMs={o.ms}
          />
        ) : (
          <Text type="supporting" color="secondary" hasTabularNumbers>
            {count}
          </Text>
        )}
        <ClearAll afterCount />
      </HStack>
    </Swap>
  );
}

export async function Results({
  outcome,
  rates,
  settings,
  filters,
  names,
  noSemantic,
}: {
  outcome: Promise<SearchOutcome>;
  rates: Promise<Rates>;
  settings: Settings;
  filters: Filters;
  names: ClauseNames;
  noSemantic: boolean;
}) {
  const o = await outcome;
  if (o.state !== "ok") return <KeepResults failure={o.state} />;
  const { result } = o;
  // this page's search went without meaning-based matches, asked to or not: the rest of the page and the later
  // pages are read the same way, so the result set stays the one the first tiles came from
  const lexical = noSemantic || (!!filters.q && result.semantic !== "used");
  const t = translator(settings.ui);
  if (result.items.length === 0)
    return (
      <KeepResults>
        <Swap>
          <EmptyResults
            // an API from before `widerCounts` leaves it out
            wider={widerSearches(filters, names, t, result.widerCounts ?? [])}
          />
        </Swap>
      </KeepResults>
    );
  const known = await rates;
  const href = (n: number) =>
    `?${pageQuery(filtersToQuery(filters), n, lexical)}`;
  return (
    <KeepResults>
      <Swap>
        <VStack gap={6}>
          {result.typoTolerant && (
            <SemanticNote text={t("list.typoTolerant")} />
          )}
          <ResultPages
            // a new search starts over, and so do new tile texts or prices (a language, item text or currency
            // change), at the place the history entry keeps; the same search read again (a retry) keeps the pages
            key={`${filtersToQuery(filters)}${lexical ? "&sem=0" : ""} ${drawnWith(settings)}`}
            page={result.page}
            total={result.totalPages}
            count={result.count}
            pageSize={result.pageSize}
            tiles={result.items
              .slice(0, FIRST_TILES)
              .map((item) => tileOf(item, settings, known, t))}
            partial={result.items.length > FIRST_TILES}
            filters={filters}
            lexical={lexical}
            settings={settings}
            rates={known}
          />
          {result.semantic === "unavailable" && (
            <SemanticNote text={t("list.semanticUnavailable")} />
          )}
          {/* plain links for crawlers and screen readers' link lists; out of the tab order, the scroll reads on */}
          <VisuallyHidden as="nav" aria-label={t("page.nav")}>
            {result.page > 1 && (
              <a href={href(result.page - 1)} tabIndex={-1}>
                {t("page.prev")}
              </a>
            )}
            {result.page < result.totalPages && (
              <a href={href(result.page + 1)} tabIndex={-1}>
                {t("page.next")}
              </a>
            )}
          </VisuallyHidden>
        </VStack>
      </Swap>
    </KeepResults>
  );
}
