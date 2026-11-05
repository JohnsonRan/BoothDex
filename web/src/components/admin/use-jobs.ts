"use client";
// Job rows and schedules from GET /api/admin/jobs (the last 100 jobs), polled every 5 s while a crawler page is
// open, as master did. `act` runs one write, toasts its result and reloads the rows.

import { useToast } from "@astryxdesign/core/Toast";
import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { Job, Schedule } from "./types";

const POLL_MS = 5000;

export function useJobs() {
  const toast = useToast();
  const [data, setData] = useState<{ jobs: Job[]; schedules: Schedule[] }>();

  const load = useCallback(() => {
    api<{ jobs: Job[]; schedules: Schedule[] }>("/api/admin/jobs").then(
      setData,
      (e: Error) => toast({ type: "error", body: e.message, uniqueID: "jobs" }),
    );
  }, [toast]);

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
        toast({
          type: "error",
          body: e instanceof Error ? e.message : String(e),
        });
        return false;
      }
    },
    [toast, load],
  );

  return { jobs: data?.jobs, schedules: data?.schedules, act };
}
