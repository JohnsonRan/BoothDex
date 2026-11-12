"use client";
// Crawler tasks: one row per job kind in the Astryx template incident-console's list
// (@astryxdesign/cli assets/templates/pages/incident-console/page.tsx:347: List, compact, with dividers). Live jobs come from the
// overview stream, schedules and history from the jobs poll. Stopping a job asks first (AlertDialog, the
// block @astryxdesign/cli assets/templates/blocks/components/AlertDialog/AlertDialogAsyncAction.tsx).

import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { List } from "@astryxdesign/core/List";
import { useState } from "react";
import { useLive } from "./admin-console";
import { JobLog } from "./job-log";
import { JobsFrame } from "./jobs-frame";
import { useAdminT } from "./messages";
import { RunDialog } from "./run-dialog";
import { TaskRow } from "./task-row";
import { type Job, KINDS, type Kind } from "./types";
import { useJobs } from "./use-jobs";

function TaskList() {
  const t = useAdminT();
  const { overview } = useLive();
  const { jobs, schedules, act } = useJobs();
  const [logId, setLogId] = useState<number>();
  const [runKind, setRunKind] = useState<Kind>();
  const [stopping, setStopping] = useState<Job>();
  const [busy, setBusy] = useState(false);

  const stop = async (job: Job) => {
    setBusy(true);
    await act(
      `/api/admin/jobs/${job.id}/cancel`,
      { method: "POST" },
      t("toast.stopRequested"),
    );
    setBusy(false);
    setStopping(undefined);
  };

  return (
    <>
      <List density="compact" hasDividers edge="outset">
        {KINDS.map((kind) => (
          <TaskRow
            key={kind}
            kind={kind}
            job={overview.live_jobs.find((j) => j.kind === kind)}
            schedule={schedules?.find((s) => s.kind === kind)}
            // jobs arrive newest first
            lastFailed={
              jobs?.find((j) => j.kind === kind && j.finished_at)?.status ===
              "failed"
            }
            act={act}
            onLog={setLogId}
            onStop={setStopping}
            onRunOptions={setRunKind}
          />
        ))}
      </List>
      {logId !== undefined && (
        <JobLog id={logId} onClose={() => setLogId(undefined)} />
      )}
      {runKind && (
        <RunDialog
          kind={runKind}
          onClose={() => setRunKind(undefined)}
          onRun={(params) =>
            act(
              "/api/admin/jobs",
              { method: "POST", json: { kind: runKind, params } },
              t("toast.queued", { kind: t(`kind.${runKind}`) }),
            )
          }
        />
      )}
      <AlertDialog
        isOpen={stopping !== undefined}
        onOpenChange={(open) => !open && setStopping(undefined)}
        title={
          stopping ? t("stop.title", { kind: t(`kind.${stopping.kind}`) }) : ""
        }
        description={t("stop.desc")}
        actionLabel={t("jobs.stop")}
        actionVariant="destructive"
        isActionLoading={busy}
        onAction={() => stopping && stop(stopping)}
      />
    </>
  );
}

export function Tasks() {
  return (
    <JobsFrame tab="tasks">
      <TaskList />
    </JobsFrame>
  );
}
