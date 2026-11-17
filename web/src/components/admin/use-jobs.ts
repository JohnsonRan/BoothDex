"use client";

// Job rows and schedules from GET /api/admin/jobs (the last 100 jobs, or with ?unresolved=true the newest 100
// unresolved failed runs and their total), polled every 5 s while a crawler page is open, as master did. `act` runs
// one write, toasts its result and reloads the rows.

import { useCallback, useEffect, useRef, useState } from "react";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import { errorText, useAdminT } from "./messages";
import type { Job, Schedule } from "./types";

const POLL_MS = 5000;

/** `total` comes only with ?unresolved=true: how many runs there are, of which `jobs` holds the newest 100. */
type JobsAnswer = { jobs: Job[]; total?: number; schedules: Schedule[] };

export function useJobs(path = "/api/admin/jobs") {
  const t = useAdminT();
  const toast = useToast();
  const [data, setData] = useState<{ path: string } & JobsAnswer>();

  const latest = useRef(path);
  latest.current = path;

  const load = useCallback(() => {
    api<JobsAnswer>(path).then(
      // an answer to a request the filter has since replaced is dropped
      (d) => latest.current === path && setData({ path, ...d }),
      (e) => toast({ type: "error", body: errorText(t, e), uniqueID: "jobs" }),
    );
  }, [path, toast, t]);

  useEffect(() => {
    load();
    const timer = setInterval(load, POLL_MS);
    return () => clearInterval(timer);
  }, [load]);

  const act = useCallback(
    async (
      path: string,
      init: RequestInit & { json?: unknown },
      ok: string,
    ) => {
      try {
        await api(path, init);
        toast({ body: ok });
        load();
        return true;
      } catch (e) {
        toast({ type: "error", body: errorText(t, e) });
        return false;
      }
    },
    [toast, load, t],
  );

  // until the new request answers, the rows of the previous one are not shown as its rows
  const current = data?.path === path ? data : undefined;
  return {
    jobs: current?.jobs,
    total: current?.total,
    schedules: current?.schedules,
    act,
  };
}
