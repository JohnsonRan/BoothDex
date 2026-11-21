"use client";
// What replaces the grid when there is nothing to show (EmptyState template blocks).

import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Center } from "@astryxdesign/core/Center";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { Link } from "@astryxdesign/core/Link";
import { Spinner } from "@astryxdesign/core/Spinner";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useRouter } from "next/navigation";
import { useEffect, useState, useTransition } from "react";
import { useAccount } from "@/components/account/session";
import {
  CLAUSE_CLEAR,
  type Wider,
} from "@/components/catalog/filter/clause-text";
import {
  useFilterUrl,
  useRefresh,
} from "@/components/catalog/filter/filter-url";
import { useUi } from "@/components/providers";
import {
  activeFilterCount,
  FILTER_RESET,
  type Filters,
  filtersToQuery,
  patched,
} from "@/lib/catalog";
import { useApiRecovery } from "@/lib/client-data";
import { LOCALES } from "@/lib/settings-core";

/** No results: with filters set, offer to drop one clause (`wider`: the ones that find something, most first) or
 *  all of them (the search itself stays); `related`, the related searches' row, offers other words. */
export function EmptyResults({
  wider,
  related,
}: {
  wider: Wider[];
  related?: React.ReactNode;
}) {
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
        wider.length > 0 || related ? (
          <VStack gap={4} hAlign="center">
            {wider.length > 0 && (
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
            )}
            {clear}
            {related}
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

/** How long a page whose search came back `pending` waits before it reads the search once more. */
export const PENDING_RETRY_MS = 1000;

/** Arms the one extra read of a `pending` search: `read` runs after `ms` unless the returned cancel runs first.
 *  Nothing is armed once the note has asked, or while a navigation is under way. */
export function armRetry(
  asked: boolean,
  navigating: boolean,
  read: () => void,
  ms = PENDING_RETRY_MS,
): (() => void) | undefined {
  if (asked || navigating) return;
  const timer = setTimeout(read, ms);
  return () => clearTimeout(timer);
}

/** The note on a search whose meaning-based matches were still being prepared (`semantic: "pending"`): the page reads
 *  the search once more, which then waits for the vector the API went on to fetch. Keyed by the search (results.tsx),
 *  so a new search drops the wait; an answer still pending shows `unavailable` and asks no more. */
export function PendingNote({
  pending,
  unavailable,
}: {
  pending: string;
  unavailable: string;
}) {
  const router = useRouter();
  const { pending: navigating } = useRefresh();
  const [asked, setAsked] = useState(false);
  const [reading, startTransition] = useTransition();
  useEffect(
    () =>
      armRetry(asked, navigating, () => {
        setAsked(true);
        startTransition(() => router.refresh());
      }),
    [asked, navigating, router],
  );
  return <SemanticNote text={asked && !reading ? unavailable : pending} />;
}

/** A link to the results after `patch`: a plain click applies it as a filter change, so the refresh shows its bar
 *  and keeps the place as any other does; the address of the same change serves a modified click or a new tab. */
function useApplyLink(patch: Partial<Filters>) {
  const { filters, apply } = useFilterUrl();
  return {
    href: `?${filtersToQuery(patched(filters, patch))}`,
    onClick: (e: React.MouseEvent) => {
      if (e.button || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      e.preventDefault();
      apply(patch);
    },
  };
}

/** A results note's "search instead" link (results.tsx). */
export function SearchInstead({
  patch,
  label,
}: {
  patch: Partial<Filters>;
  label: string;
}) {
  return <Link {...useApplyLink(patch)}>{label}</Link>;
}

/** A related search (results.tsx): its query searched instead, the other filters kept. */
export function RelatedSearchLink({ q, label }: { q: string; label: string }) {
  return (
    <Button
      {...useApplyLink({ q })}
      label={label}
      size="sm"
      icon={<Icon icon="search" />}
    />
  );
}
