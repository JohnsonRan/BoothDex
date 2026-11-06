"use client";
// One job's details and log: GET /api/admin/jobs/{id}, polled every 2 s while the job is live. The Astryx block
// Dialog — Scrolling content (templates/blocks/components/Dialog/DialogScrollingContent.tsx) holds the facts as
// the template incident-console's inspector lists them (templates/pages/incident-console/page.tsx:423-439:
// MetadataList with the labels at the start), the error as an error Banner and the log in a CodeBlock.

import { Banner } from "@astryxdesign/core/Banner";
import { CodeBlock } from "@astryxdesign/core/CodeBlock";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Layout, LayoutContent, VStack } from "@astryxdesign/core/Layout";
import {
  MetadataList,
  MetadataListItem,
} from "@astryxdesign/core/MetadataList";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Text } from "@astryxdesign/core/Text";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { useEffect, useState } from "react";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import { LOCALES } from "@/lib/settings-core";
import { formatNumber, jobDuration } from "./format";
import { JobStatusToken } from "./job-status";
import { errorText, useAdminT } from "./messages";
import type { Job } from "./types";

const POLL_MS = 2000;
const finished = (j: Job) => j.finished_at !== null;

export function JobLog({ id, onClose }: { id: number; onClose: () => void }) {
  const t = useAdminT();
  const locale = LOCALES[useUi().ui];
  const [job, setJob] = useState<Job>();
  // the failure, worded at render in the current language
  const [error, setError] = useState<unknown>();

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;
    let stopped = false;
    // a failed read is shown and tried again on the same beat; the next good one clears it
    const load = () =>
      api<Job>(`/api/admin/jobs/${id}`).then(
        (j) => {
          if (stopped) return;
          setJob(j);
          setError(undefined);
          if (!finished(j)) timer = setTimeout(load, POLL_MS);
        },
        (err) => {
          if (stopped) return;
          setError(err);
          timer = setTimeout(load, POLL_MS);
        },
      );
    load();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [id]);

  return (
    <Dialog isOpen onOpenChange={onClose} width={720} maxHeight="85dvh">
      <Layout
        header={
          <DialogHeader
            title={job ? `#${job.id} ${t(`kind.${job.kind}`)}` : `#${id}`}
            endContent={job && <JobStatusToken status={job.status} />}
            onOpenChange={onClose}
          />
        }
        content={
          <LayoutContent>
            {error ? (
              <Banner status="error" title={errorText(t, error)} />
            ) : !job ? (
              <Spinner label={t("log.output")} />
            ) : (
              <VStack gap={4} hAlign="stretch">
                <MetadataList
                  columns="single"
                  label={{ position: "start", width: 96 }}
                >
                  <MetadataListItem label={t("log.progress")}>
                    <Text type="body">
                      {formatNumber(job.done, locale)}
                      {job.total ? ` / ${formatNumber(job.total, locale)}` : ""}
                      {job.message && ` · ${job.message}`}
                    </Text>
                  </MetadataListItem>
                  <MetadataListItem label={t("log.duration")}>
                    <Text type="body">{jobDuration(job, t)}</Text>
                  </MetadataListItem>
                  <MetadataListItem label={t("log.creator")}>
                    <Text type="body">{job.created_by}</Text>
                  </MetadataListItem>
                  <MetadataListItem label={t("log.created")}>
                    <Timestamp
                      value={job.created_at}
                      format="date_time"
                      type="body"
                    />
                  </MetadataListItem>
                  <MetadataListItem label={t("log.params")}>
                    <Text type="code">{JSON.stringify(job.params)}</Text>
                  </MetadataListItem>
                </MetadataList>
                {job.error && (
                  <Banner
                    status="error"
                    title={t("log.error")}
                    description={job.error}
                  />
                )}
                <CodeBlock
                  title={t("log.output")}
                  code={job.log || t("log.empty")}
                  maxHeight="40dvh"
                />
              </VStack>
            )}
          </LayoutContent>
        }
      />
    </Dialog>
  );
}
