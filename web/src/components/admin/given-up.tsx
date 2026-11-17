"use client";
// The items AI processing gave up on (GET /api/admin/enrichment), a tab of the crawler page beside the enrich task
// they belong to. Rows as the history tab's (history.tsx): a List with dividers, the title linking to the item on
// the site, its id, shop and failure count, and Retry at the end. "Retry all" is the view's one primary action; it
// spends AI provider calls on every item, so it asks first (AlertDialog, as stopping a job does). GitHub Actions
// offers the same pair, a specific job or all failed jobs
// (https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs). A retry acts at once
// and reloads the list, so a row another admin already retried leaves it too.

import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { Button } from "@astryxdesign/core/Button";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, StackItem } from "@astryxdesign/core/Layout";
import { Link } from "@astryxdesign/core/Link";
import { List, ListItem } from "@astryxdesign/core/List";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Text } from "@astryxdesign/core/Text";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import { JobsFrame } from "./jobs-frame";
import { type AdminT, errorText, useAdminT } from "./messages";
import type { GivenUpItems } from "./types";

/** The retry in flight: one item's, or all of them. */
type Busy = number | "all";

export function GivenUpList({
  page,
  busy,
  onRetry,
  onRetryAll,
  t,
}: {
  page: GivenUpItems;
  busy?: Busy;
  onRetry: (id: number) => void;
  onRetryAll: () => void;
  t: AdminT;
}) {
  const shown = page.items.length;
  return (
    <>
      <Text color="secondary">{t("givenUp.desc", { n: page.attempts })}</Text>
      {shown === 0 ? (
        <EmptyState
          title={t("givenUp.empty")}
          icon={<Icon icon="admin:empty" size="lg" />}
        />
      ) : (
        <>
          <HStack gap={2} vAlign="center" wrap="wrap">
            <Text type="supporting" color="secondary">
              {shown < page.total
                ? t("givenUp.shown", { shown, n: page.total })
                : t("givenUp.count", { n: page.total })}
            </Text>
            <StackItem size="fill" />
            <Button
              label={t("givenUp.retryAll")}
              isLoading={busy === "all"}
              isDisabled={busy !== undefined && busy !== "all"}
              onClick={onRetryAll}
            />
          </HStack>
          <List density="compact" hasDividers edge="outset">
            {page.items.map((it) => (
              <ListItem
                key={it.id}
                label={<Link href={`/?item=${it.id}`}>{it.title}</Link>}
                // a node, not a string, so a long shop name wraps on a phone instead of cutting off the count
                description={
                  <Text type="supporting" color="secondary">
                    {[
                      `#${it.id}`,
                      it.author_name,
                      t("givenUp.failures", { n: it.failures }),
                    ].join(" · ")}
                  </Text>
                }
                endContent={
                  <Button
                    label={t("givenUp.retry")}
                    variant="secondary"
                    size="sm"
                    isLoading={busy === it.id}
                    isDisabled={busy !== undefined && busy !== it.id}
                    onClick={() => onRetry(it.id)}
                  />
                }
              />
            ))}
          </List>
        </>
      )}
    </>
  );
}

function GivenUp() {
  const t = useAdminT();
  const toast = useToast();
  const [page, setPage] = useState<GivenUpItems>();
  const [error, setError] = useState<unknown>();
  const [busy, setBusy] = useState<Busy>();
  const [confirming, setConfirming] = useState(false);

  const load = useCallback(() => {
    setError(undefined);
    return api<GivenUpItems>("/api/admin/enrichment").then(setPage, setError);
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const retry = async (target: Busy) => {
    setBusy(target);
    try {
      const { reset } = await api<{ reset: number }>(
        target === "all"
          ? "/api/admin/enrichment/retry"
          : `/api/admin/enrichment/${target}/retry`,
        { method: "POST" },
      );
      toast({
        body: !reset
          ? t("givenUp.stale")
          : target === "all"
            ? t("givenUp.retried", { n: reset })
            : t("givenUp.retriedOne"),
      });
      await load();
    } catch (e) {
      toast({ type: "error", body: errorText(t, e) });
    } finally {
      setBusy(undefined);
      setConfirming(false);
    }
  };

  if (error)
    return (
      <EmptyState
        title={errorText(t, error)}
        icon={<Icon icon="admin:failed" size="lg" />}
        actions={<Button label={t("gate.retry")} onClick={load} />}
      />
    );
  if (!page) return <Spinner label={t("jobs.givenUp")} />;
  return (
    <>
      <GivenUpList
        page={page}
        busy={busy}
        onRetry={retry}
        onRetryAll={() => setConfirming(true)}
        t={t}
      />
      <AlertDialog
        isOpen={confirming}
        onOpenChange={(open) =>
          !open && busy === undefined && setConfirming(false)
        }
        title={t("givenUp.retryAllTitle", { n: page.total })}
        description={t("givenUp.retryAllDesc", { attempts: page.attempts })}
        actionLabel={t("givenUp.retryAll")}
        isActionLoading={busy === "all"}
        onAction={() => retry("all")}
      />
    </>
  );
}

export function GivenUpPage() {
  return (
    <JobsFrame tab="givenUp">
      <GivenUp />
    </JobsFrame>
  );
}
