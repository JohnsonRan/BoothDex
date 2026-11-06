"use client";
// The overview's daily figures from GET /api/admin/trends, reloaded every minute: they change by the day, and
// the query scans a month of items, so the 2 s stream does not carry them.

import { useEffect, useState } from "react";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import { errorText, useAdminT } from "./messages";
import type { Trends } from "./types";

const POLL_MS = 60_000;

export function useTrends() {
  const t = useAdminT();
  const toast = useToast();
  const [trends, setTrends] = useState<Trends>();
  useEffect(() => {
    const load = () =>
      api<Trends>("/api/admin/trends").then(setTrends, (e) =>
        toast({ type: "error", body: errorText(t, e), uniqueID: "trends" }),
      );
    load();
    const timer = setInterval(load, POLL_MS);
    return () => clearInterval(timer);
  }, [toast, t]);
  return trends;
}
