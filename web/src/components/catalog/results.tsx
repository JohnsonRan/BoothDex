// The parts of the home page that wait for the search: the result count in the filter row's readout and the
// results region. Both read the same pending search, each behind its own Suspense boundary (home-screen.tsx), so the
// frame around them is painted first.

import { VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { type Filters, searchQuery } from "@/lib/catalog";
import { translator } from "@/lib/dictionaries";
import type { SearchOutcome } from "@/lib/server-data";
import { LOCALES, type Rates, type Settings } from "@/lib/settings-core";
import { LazyPager } from "./lazy-pager";
import { KeepResults, Swap } from "./refresh";
import { EmptyResults, SemanticNote } from "./result-states";
import { ResultsGrid } from "./results-grid";

export async function ResultCount({
  outcome,
  settings,
}: {
  outcome: Promise<SearchOutcome>;
  settings: Settings;
}) {
  const o = await outcome;
  if (o.state !== "ok") return null;
  const t = translator(settings.ui);
  return (
    <Swap inline>
      <Text type="supporting" color="secondary" hasTabularNumbers>
        {t("title.count", {
          n: o.result.count.toLocaleString(LOCALES[settings.ui]),
        })}
      </Text>
    </Swap>
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
