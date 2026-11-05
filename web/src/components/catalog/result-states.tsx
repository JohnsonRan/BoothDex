"use client";
// What replaces the grid when there is nothing to show (EmptyState template blocks).

import { Button } from "@astryxdesign/core/Button";
import { Center } from "@astryxdesign/core/Center";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Text } from "@astryxdesign/core/Text";
import { useRouter } from "next/navigation";
import { useUi } from "@/components/providers";
import { activeFilterCount, FILTER_RESET } from "@/lib/catalog";
import { useApiRecovery } from "@/lib/client-data";
import { useFilterUrl } from "./filter-url";

/** No results: with filters set, offer to clear them (the search itself stays). */
export function EmptyResults() {
  const { t } = useUi();
  const { filters, apply } = useFilterUrl();
  const narrowed = activeFilterCount(filters) > 0;
  return (
    <EmptyState
      icon={<Icon icon="search" size="lg" />}
      title={t("list.empty")}
      description={narrowed ? t("list.emptyHint") : undefined}
      actions={
        narrowed && (
          <Button
            label={t("filter.clear")}
            variant="primary"
            onClick={() => apply(FILTER_RESET)}
          />
        )
      }
    />
  );
}

/** The API is starting (waits and reloads by itself) or failed (retry). */
export function FailedResults({ starting }: { starting: boolean }) {
  const { t } = useUi();
  const router = useRouter();
  useApiRecovery(starting);
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
          onClick={() => router.refresh()}
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
