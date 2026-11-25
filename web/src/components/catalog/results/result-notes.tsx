// Server-rendered notes drawn alongside search results: typo explanation, cross-language expansion note and
// related-search chips. Kept in a Server Component module so their message keys stay out of the browser bundle.

import { Center } from "@astryxdesign/core/Center";
import { HStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import type { ClauseNames } from "@/components/catalog/filter/clause-text";
import type { RelatedSearch, SearchResponse } from "@/lib/api-types";
import type { Filters } from "@/lib/catalog";
import type { Translate } from "@/lib/settings-core";
import {
  RelatedSearchLink,
  SearchInstead,
  SemanticNote,
} from "./result-states";

/** The results allowed a typo. When a model was read from a misspelt word, the note names the model rather than
 *  saying the words matched nothing as typed, which the other words may have; the link searches the words as typed,
 *  as Google's "Search instead for". */
export function TypoNote({
  result: { nameTypo, recognized },
  avatars,
  filters,
  t,
}: {
  result: Pick<SearchResponse, "nameTypo" | "recognized">;
  avatars: ClauseNames["avatars"];
  filters: Filters;
  t: Translate;
}) {
  if (!nameTypo) return <SemanticNote text={t("list.typoTolerant")} />;
  // an API from before `typo` leaves it out, and one from before `recognized` leaves that out
  const misread = (recognized ?? []).filter((r) => r.typo);
  const text =
    misread.length === 0
      ? t("list.typoTolerant")
      : misread
          .map(({ slug, words }) =>
            t("list.typoModel", {
              word: words.join(" "),
              model: avatars.find(([s]) => s === slug)?.[1] ?? slug,
            }),
          )
          .join(" ");
  // the text is drawn here; only the link, a filter change, needs the browser
  return (
    <Center>
      <Text type="supporting">
        {text}{" "}
        <SearchInstead
          patch={{ noTypo: true }}
          label={t("list.searchInstead", { q: filters.q })}
        />
      </Text>
    </Center>
  );
}

/** The results include what the BOOTH tags that translate the query found (`words`); the link searches the words
 *  alone. */
export function ExpansionNote({ words, t }: { words: string[]; t: Translate }) {
  return (
    <Center>
      <Text type="supporting">
        {t("list.expandedWith", { words: words.join(" · ") })}{" "}
        <SearchInstead
          patch={{ noExpand: true }}
          label={t("list.withoutExpanded")}
        />
      </Text>
    </Center>
  );
}

/** Related searches (`/api/related`), drawn with the results, so the grid does not move for them. */
export function RelatedSearches({
  searches,
  t,
}: {
  searches: RelatedSearch[];
  t: Translate;
}) {
  return (
    <HStack gap={2} hAlign="center" vAlign="center" wrap="wrap">
      <Text type="supporting">{t("list.related")}</Text>
      {searches.map(({ q, label }) => (
        <RelatedSearchLink key={q} q={q} label={label} />
      ))}
    </HStack>
  );
}
