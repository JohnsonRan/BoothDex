// The parts of the home page that wait for the search: the result count beside the title and the results
// region. Both read the same pending search, each behind its own Suspense boundary (home-screen.tsx), so the
// frame around them is painted first.

import { VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import type { Filters } from "@/lib/catalog";
import type { SearchOutcome } from "@/lib/server-data";
import {
  LOCALES,
  type Rates,
  type Settings,
  translator,
} from "@/lib/settings-core";
import { Pager } from "./pager";
import { KeepResults } from "./refresh";
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
    <Text type="supporting" hasTabularNumbers>
      {t("title.count", {
        n: o.result.count.toLocaleString(LOCALES[settings.ui]),
      })}
    </Text>
  );
}

export async function Results({
  outcome,
  rates,
  settings,
  filters,
}: {
  outcome: Promise<SearchOutcome>;
  rates: Promise<Rates>;
  settings: Settings;
  filters: Filters;
}) {
  const o = await outcome;
  if (o.state !== "ok") return <KeepResults failure={o.state} />;
  const { result } = o;
  if (result.items.length === 0)
    return (
      <KeepResults>
        <EmptyResults />
      </KeepResults>
    );
  return (
    <KeepResults>
      <VStack gap={6}>
        <ResultsGrid
          items={result.items}
          settings={settings}
          rates={await rates}
        />
        {result.semantic === "unavailable" && (
          <SemanticNote
            text={translator(settings.ui)("list.semanticUnavailable")}
          />
        )}
        <Pager
          page={result.page}
          total={result.totalPages}
          noSemantic={!!filters.q && result.semantic !== "used"}
        />
      </VStack>
    </KeepResults>
  );
}
