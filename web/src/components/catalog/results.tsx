// The parts of the home page that wait for the search: the result count in the filter row's readout, with the
// readout's "Clear all" so the two arrive together, and the results region. Both read the same pending search,
// each behind its own Suspense boundary (home-screen.tsx), so the frame around them is painted first.

import { VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { type Filters, searchQuery } from "@/lib/catalog";
import { translator } from "@/lib/dictionaries";
import type { SearchOutcome } from "@/lib/server-data";
import { LOCALES, type Rates, type Settings } from "@/lib/settings-core";
import { ClearAll } from "./filter-bar";
import { LazyPager } from "./lazy-pager";
import { KeepResults, Swap } from "./refresh";
import { EmptyResults, SemanticNote } from "./result-states";
import { ResultsGrid } from "./results-grid";
import { SearchTrace } from "./search-trace";

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
    <>
      <Swap>
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
      </Swap>
      <ClearAll afterCount />
    </>
  );
}

export async function Results({
  outcome,
  rates,
  settings,
  filters,
  page,
  noSemantic,
}: {
  outcome: Promise<SearchOutcome>;
  rates: Promise<Rates>;
  settings: Settings;
  filters: Filters;
  page: number;
  noSemantic: boolean;
}) {
  const o = await outcome;
  if (o.state !== "ok") return <KeepResults failure={o.state} />;
  const { result } = o;
  if (result.items.length === 0)
    return (
      <KeepResults>
        <Swap>
          <EmptyResults />
        </Swap>
      </KeepResults>
    );
  return (
    <KeepResults>
      <Swap>
        <VStack gap={6}>
          <ResultsGrid
            items={result.items}
            settings={settings}
            rates={await rates}
            t={translator(settings.ui)}
            // the request searchItems (lib/server-data) made, so the API answers it from its cache
            search={`/api/search?${searchQuery(filters, page, result.pageSize)}&fields=card${noSemantic ? "&sem=0" : ""}`}
          />
          {result.semantic === "unavailable" && (
            <SemanticNote
              text={translator(settings.ui)("list.semanticUnavailable")}
            />
          )}
          <LazyPager
            page={result.page}
            total={result.totalPages}
            noSemantic={!!filters.q && result.semantic !== "used"}
          />
        </VStack>
      </Swap>
    </KeepResults>
  );
}
