"use client";
// Crawler history: the API's last 100 jobs, filtered by kind and status in the browser, with the filters in the
// URL (?kind=&status=) so a view can be shared and Back restores it. Rows are the Astryx template
// incident-console's (templates/pages/incident-console/page.tsx:347-380: List with dividers, a status Token and
// a relative Timestamp at the end, the row opens the details), and its EmptyState (:325-332) when nothing is
// left. The filter Selectors sit above the list, as table-filter keeps its controls over the table.

import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, StackItem } from "@astryxdesign/core/Layout";
import { List, ListItem } from "@astryxdesign/core/List";
import { Selector } from "@astryxdesign/core/Selector";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Text } from "@astryxdesign/core/Text";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useState } from "react";
import { jobDuration } from "./format";
import { JobLog } from "./job-log";
import { JobStatusToken } from "./job-status";
import { JobsFrame } from "./jobs-frame";
import { useAdminT } from "./messages";
import { KINDS, STATUSES } from "./types";
import { useJobs } from "./use-jobs";

const ALL = "all";

export function History() {
  const t = useAdminT();
  const router = useRouter();
  const path = usePathname();
  const query = useSearchParams();
  const { jobs } = useJobs();
  const [logId, setLogId] = useState<number>();
  const kind = query.get("kind") ?? ALL;
  const status = query.get("status") ?? ALL;

  const filter = (name: "kind" | "status", value: string) => {
    const next = new URLSearchParams(query);
    if (value === ALL) next.delete(name);
    else next.set(name, value);
    const qs = next.toString();
    router.replace(qs ? `${path}?${qs}` : path, { scroll: false });
  };

  const rows = jobs?.filter(
    (j) =>
      (kind === ALL || j.kind === kind) &&
      (status === ALL || j.status === status),
  );
  const all = { value: ALL, label: t("history.all") };

  return (
    <JobsFrame tab="history">
      <HStack gap={2} vAlign="center" wrap="wrap">
        <Selector
          label={t("history.kind")}
          size="sm"
          variant="secondary"
          value={kind}
          options={[
            all,
            ...KINDS.map((k) => ({ value: k, label: t(`kind.${k}`) })),
          ]}
          onChange={(v) => filter("kind", v)}
        />
        <Selector
          label={t("history.status")}
          size="sm"
          variant="secondary"
          value={status}
          options={[
            all,
            ...STATUSES.map((s) => ({ value: s, label: t(`status.${s}`) })),
          ]}
          onChange={(v) => filter("status", v)}
        />
        <StackItem size="fill" />
        {jobs && (
          <Text type="supporting" color="secondary">
            {t("history.count", { n: jobs.length })}
          </Text>
        )}
      </HStack>
      {!rows ? (
        <Spinner label={t("jobs.history")} />
      ) : rows.length === 0 ? (
        <EmptyState
          title={jobs?.length ? t("history.noMatch") : t("history.empty")}
          icon={<Icon icon="admin:empty" size="lg" />}
        />
      ) : (
        <List density="compact" hasDividers>
          {rows.map((j) => (
            <ListItem
              key={j.id}
              label={`#${j.id} ${t(`kind.${j.kind}`)}`}
              description={[
                j.error ?? j.message,
                jobDuration(j, t),
                j.created_by,
              ]
                .filter(Boolean)
                .join(" · ")}
              endContent={
                <HStack gap={3} vAlign="center">
                  <JobStatusToken status={j.status} />
                  <Timestamp
                    value={j.created_at}
                    format="relative"
                    color="secondary"
                  />
                </HStack>
              }
              onClick={() => setLogId(j.id)}
            />
          ))}
        </List>
      )}
      {logId !== undefined && (
        <JobLog id={logId} onClose={() => setLogId(undefined)} />
      )}
    </JobsFrame>
  );
}
