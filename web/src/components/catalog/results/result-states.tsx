"use client";
// What replaces the grid when there is nothing to show (EmptyState template blocks).

import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Center } from "@astryxdesign/core/Center";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { Spinner } from "@astryxdesign/core/Spinner";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useRouter } from "next/navigation";
import { useAccount } from "@/components/account/session";
import {
  CLAUSE_CLEAR,
  type Wider,
} from "@/components/catalog/filter/clause-text";
import { useFilterUrl } from "@/components/catalog/filter/filter-url";
import { useUi } from "@/components/providers";
import { activeFilterCount, FILTER_RESET } from "@/lib/catalog";
import { useApiRecovery } from "@/lib/client-data";
import { LOCALES } from "@/lib/settings-core";

/** No results: with filters set, offer to drop one clause (`wider`: the ones that find something, most first) or
 *  all of them (the search itself stays). */
export function EmptyResults({ wider }: { wider: Wider[] }) {
  const { t, ui } = useUi();
  const { filters, apply } = useFilterUrl();
  const narrowed = activeFilterCount(filters) > 0;
  const clear = narrowed && (
    <Button
      label={t("filter.clear")}
      variant="primary"
      onClick={() => apply(FILTER_RESET)}
    />
  );
  return (
    <EmptyState
      icon={<Icon icon="search" size="lg" />}
      title={t("list.empty")}
      description={
        wider.length > 0
          ? t("list.emptyWider")
          : narrowed
            ? t("list.emptyHint")
            : undefined
      }
      actions={
        wider.length > 0 ? (
          <VStack gap={4} hAlign="center">
            <HStack gap={2} hAlign="center" wrap="wrap">
              {wider.map(({ key, label, count }) => {
                const n = t("title.count", {
                  n: count.toLocaleString(LOCALES[ui]),
                });
                return (
                  <Button
                    key={key}
                    label={t("list.dropClause", { clause: label, count: n })}
                    tooltip={label}
                    icon={<Icon icon="close" />}
                    endContent={<Badge label={n} />}
                    onClick={() => apply(CLAUSE_CLEAR[key])}
                  >
                    {label}
                  </Button>
                );
              })}
            </HStack>
            {clear}
          </VStack>
        ) : (
          clear
        )
      }
    />
  );
}

/** The API is starting (waits and reloads by itself) or failed (retry: the page again, or `onRetry`). */
export function FailedResults({
  starting,
  onRetry,
}: {
  starting: boolean;
  /** for results read in the browser (the recent view) */
  onRetry?: () => void;
}) {
  const { t } = useUi();
  const router = useRouter();
  useApiRecovery(starting, useAccount().reload);
  return starting ? (
    <EmptyState
      icon={<Spinner size="xl" />}
      title={t("list.startingTitle")}
      description={t("list.starting")}
    />
  ) : (
    <EmptyState
      icon={<Icon icon="error" size="lg" />}
      title={t("list.loadFailed")}
      actions={
        <Button
          label={t("list.retry")}
          variant="primary"
          onClick={onRetry ?? (() => router.refresh())}
        />
      }
    />
  );
}

export function SemanticNote({ text }: { text: string }) {
  return (
    <Center>
      <Text type="supporting">{text}</Text>
    </Center>
  );
}
