"use client";
// One crawler job kind as a row of the Astryx template incident-console's list
// (templates/pages/incident-console/page.tsx:347-380): name and status Token on the first line, then the kind's
// description, its progress or next run, and its controls (schedule Switch, interval Selector, Run or Log and
// Stop, and a MoreMenu with the run options), which wrap under the text on a phone.

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Layout";
import { ListItem } from "@astryxdesign/core/List";
import { MoreMenu } from "@astryxdesign/core/MoreMenu";
import { ProgressBar } from "@astryxdesign/core/ProgressBar";
import { Selector } from "@astryxdesign/core/Selector";
import { Switch } from "@astryxdesign/core/Switch";
import { Text } from "@astryxdesign/core/Text";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { Token } from "@astryxdesign/core/Token";
import { useUi } from "@/components/providers";
import { LOCALES } from "@/lib/settings-core";
import { formatNumber, intervalLabel, jobDuration, jobEta } from "./format";
import { JobStatusToken } from "./job-status";
import { useAdminT } from "./messages";
import {
  INTERVALS,
  type Job,
  type Kind,
  RUN_PARAMS,
  type Schedule,
} from "./types";
import type { useJobs } from "./use-jobs";

export function TaskRow({
  kind,
  job,
  schedule,
  lastFailed,
  act,
  onLog,
  onStop,
  onRunOptions,
}: {
  kind: Kind;
  /** the kind's queued or running job, from the live overview */
  job?: Job;
  schedule?: Schedule;
  lastFailed: boolean;
  act: ReturnType<typeof useJobs>["act"];
  onLog: (id: number) => void;
  onStop: (job: Job) => void;
  onRunOptions: (kind: Kind) => void;
}) {
  const t = useAdminT();
  const locale = LOCALES[useUi().ui];
  const name = t(`kind.${kind}`);
  const put = (json: object, ok: string) =>
    act(`/api/admin/schedules/${kind}`, { method: "PUT", json }, ok);
  const active = job?.status === "running" || job?.status === "cancelling";
  const eta = job && jobEta(job, t);

  const more = [
    ...(RUN_PARAMS[kind]
      ? [{ label: t("jobs.runOptions"), onClick: () => onRunOptions(kind) }]
      : []),
    ...(schedule
      ? [
          {
            label: t("jobs.runSchedule"),
            onClick: () =>
              put({ run_now: true }, t("toast.queued", { kind: name })),
          },
        ]
      : []),
  ];

  return (
    <ListItem
      label={
        <HStack gap={2} vAlign="center" wrap="wrap">
          <Text type="label">{name}</Text>
          {job && <JobStatusToken status={job.status} />}
          {!job && lastFailed && (
            <Token size="sm" color="red" label={t("jobs.lastFailed")} />
          )}
        </HStack>
      }
      description={
        <VStack gap={2}>
          <Text type="supporting" color="secondary">
            {t(`kind.${kind}.desc`)}
          </Text>
          {job && active && (
            <VStack gap={1}>
              <ProgressBar
                label={name}
                isLabelHidden
                value={job.total ? job.done : undefined}
                max={job.total ?? undefined}
                isIndeterminate={!job.total}
              />
              <Text type="supporting" color="secondary">
                {job.message ?? t("jobs.starting")} ·{" "}
                {formatNumber(job.done, locale)}
                {job.total ? ` / ${formatNumber(job.total, locale)}` : ""} ·{" "}
                {t("jobs.elapsed", { d: jobDuration(job, t) })}
                {eta && ` · ${t("jobs.eta", { d: eta })}`}
              </Text>
            </VStack>
          )}
          {!job && schedule?.enabled && (
            <Text type="supporting" color="secondary">
              {t("jobs.next")}{" "}
              <Timestamp
                value={schedule.next_run_at}
                format="date_time"
                type="supporting"
                color="secondary"
              />
            </Text>
          )}
          <HStack gap={2} vAlign="center" wrap="wrap">
            {schedule && (
              <>
                <Switch
                  label={t("jobs.schedule")}
                  size="sm"
                  value={schedule.enabled}
                  changeAction={(v) =>
                    put(
                      { enabled: v },
                      t(v ? "toast.scheduleOn" : "toast.scheduleOff"),
                    ).then(() => {})
                  }
                />
                <Selector
                  label={t("jobs.interval")}
                  isLabelHidden
                  size="sm"
                  value={String(schedule.interval_minutes)}
                  options={INTERVALS.map((m) => ({
                    value: String(m),
                    label: intervalLabel(m, t),
                  }))}
                  changeAction={(v) =>
                    put(
                      { interval_minutes: Number(v) },
                      t("toast.interval"),
                    ).then(() => {})
                  }
                />
              </>
            )}
            <StackItem size="fill" />
            {job ? (
              <>
                <Button
                  label={t("jobs.log")}
                  variant="secondary"
                  size="sm"
                  icon={<Icon icon="admin:log" size="sm" />}
                  onClick={() => onLog(job.id)}
                />
                <Button
                  label={t("jobs.stop")}
                  variant="destructive"
                  size="sm"
                  icon={<Icon icon="admin:stop" size="sm" />}
                  isDisabled={job.status === "cancelling"}
                  onClick={() => onStop(job)}
                />
              </>
            ) : (
              <Button
                label={t("jobs.run")}
                variant="primary"
                size="sm"
                icon={<Icon icon="admin:run" size="sm" />}
                onClick={() =>
                  act(
                    "/api/admin/jobs",
                    { method: "POST", json: { kind, params: {} } },
                    t("toast.queued", { kind: name }),
                  )
                }
              />
            )}
            {!job && more.length > 0 && (
              <MoreMenu size="sm" label={t("jobs.more")} items={more} />
            )}
          </HStack>
        </VStack>
      }
    />
  );
}
