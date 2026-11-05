// Numbers, sizes and durations in the interface language. Points in time use Astryx Timestamp instead.

import type { AdminT } from "./messages";
import type { Job } from "./types";

const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(n: number | null | undefined, locale: string) {
  if (n == null) return "—";
  let v = n;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  const digits = v >= 100 || i === 0 ? 0 : 1;
  return `${v.toLocaleString(locale, { maximumFractionDigits: digits })} ${UNITS[i]}`;
}

export const formatNumber = (n: number | null | undefined, locale: string) =>
  n == null ? "—" : n.toLocaleString(locale);

export function formatSeconds(total: number, t: AdminT) {
  const s = Math.max(0, Math.round(total));
  if (s < 60) return t("dur.s", { s });
  if (s < 3600) return t("dur.m", { m: Math.floor(s / 60), s: s % 60 });
  return t("dur.h", {
    h: Math.floor(s / 3600),
    m: Math.floor((s % 3600) / 60),
  });
}

/** From start to finish, or to now while the job runs. */
export function jobDuration(job: Job, t: AdminT, now = Date.now()) {
  if (!job.started_at) return "—";
  const end = job.finished_at ? Date.parse(job.finished_at) : now;
  return formatSeconds((end - Date.parse(job.started_at)) / 1000, t);
}

/** Time left at the job's average pace so far; null until there is a pace to go by. */
export function jobEta(job: Job, t: AdminT, now = Date.now()) {
  if (!job.total || job.done <= 0 || !job.started_at) return null;
  const spent = (now - Date.parse(job.started_at)) / 1000;
  return formatSeconds((spent / job.done) * (job.total - job.done), t);
}

export function intervalLabel(minutes: number, t: AdminT) {
  if (minutes === 10080) return t("interval.week");
  if (minutes === 1440) return t("interval.day");
  if (minutes < 60) return t("interval.min", { n: minutes });
  return t("interval.hour", { n: minutes / 60 });
}
