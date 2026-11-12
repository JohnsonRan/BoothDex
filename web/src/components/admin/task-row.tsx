"use client";
// One crawler job kind as a row of the Astryx template incident-console's list
// (@astryxdesign/cli assets/templates/pages/incident-console/page.tsx:347-380): name and status Token on the first line, then the kind's
// description and its progress or next run; the controls sit in the row's end slot, centred on the row, as the
// template puts its Token and Timestamp there. Every row ends the same way, so the buttons line up: the schedule
// Switch and interval Selector where the kind has a schedule, one small secondary button (Run, or Stop while a
// job is live) and the MoreMenu (run options, or the live job's log); a menu with nothing to offer stays in
// place, disabled. On a phone, where AppShell turns the nav into a drawer, the controls move under the
// description as one line: the Switch drops its visible label (the interval beside it says what it schedules)
// so the schedule and the buttons fit side by side.

import { useAppShellMobile } from "@astryxdesign/core/AppShell";
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

/** one width for every row's interval Selector on a desktop, so the schedule controls form a column */
const INTERVAL_WIDTH = "7.5rem";

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
  const isMobile = useAppShellMobile().isMobile;
  const name = t(`kind.${kind}`);
  const put = (json: object, ok: string) =>
    act(`/api/admin/schedules/${kind}`, { method: "PUT", json }, ok);
  const active = job?.status === "running" || job?.status === "cancelling";
  const eta = job && jobEta(job, t);

  const more = job
    ? [{ label: t("jobs.log"), onClick: () => onLog(job.id) }]
    : [
        ...(RUN_PARAMS[kind]
          ? [{ label: t("jobs.runOptions"), onClick: () => onRunOptions(kind) }]
          : []),
        // a one-off job with the schedule's settings, whether the schedule is on or off
        ...(schedule
          ? [
              {
                label: t("jobs.runSchedule"),
                onClick: () =>
                  act(
                    "/api/admin/jobs",
                    { method: "POST", json: { kind, params: schedule.params } },
                    t("toast.queued", { kind: name }),
                  ),
              },
            ]
          : []),
      ];

  const controls = (
    <HStack gap={2} vAlign="center" wrap="wrap">
      {schedule && (
        <HStack gap={2} vAlign="center" wrap="nowrap">
          <Switch
            label={t("jobs.schedule")}
            isLabelHidden={isMobile}
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
            width={isMobile ? undefined : INTERVAL_WIDTH}
            value={String(schedule.interval_minutes)}
            options={INTERVALS.map((m) => ({
              value: String(m),
              label: intervalLabel(m, t),
            }))}
            changeAction={(v) =>
              put({ interval_minutes: Number(v) }, t("toast.interval")).then(
                () => {},
              )
            }
          />
        </HStack>
      )}
      {isMobile && <StackItem size="fill" />}
      <HStack gap={1} vAlign="center" wrap="nowrap">
        {job ? (
          <Button
            label={t("jobs.stop")}
            variant="secondary"
            size="sm"
            icon={<Icon icon="admin:stop" size="sm" />}
            isDisabled={job.status === "cancelling"}
            onClick={() => onStop(job)}
          />
        ) : (
          <Button
            label={t("jobs.run")}
            variant="secondary"
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
        <MoreMenu
          size="sm"
          label={t("jobs.more")}
          items={more}
          isDisabled={more.length === 0}
        />
      </HStack>
    </HStack>
  );

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
          {isMobile && controls}
        </VStack>
      }
      endContent={isMobile ? undefined : controls}
    />
  );
}
