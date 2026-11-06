"use client";
// The overview's crawler band: the worker, what runs now, Booth's answers since the worker started and the
// month's failed runs, then two daily charts: finished runs by outcome (one kind or all, picked as the template
// dashboard-alert-rail picks its scope: a Selector with a hidden label, page.tsx:1482-1489) and the item pages
// the details and refresh runs fetched. Outcome colours are categorical, not status, so they stay apart for
// colour-blind readers; the legend and tooltip name them.

import { Selector } from "@astryxdesign/core/Selector";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { LOCALES } from "@/lib/settings-core";
import { useLive } from "./admin-console";
import { Band, Charts, Tiles } from "./band";
import { BarChart } from "./bar-chart";
import { formatBytes, formatNumber } from "./format";
import { useAdminT } from "./messages";
import { MetricCard } from "./metric-card";
import { type JobStatus, KINDS, type Trends } from "./types";

const OUTCOMES: { status: JobStatus; color: string }[] = [
  { status: "succeeded", color: "var(--color-data-categorical-blue)" },
  { status: "failed", color: "var(--color-data-categorical-orange)" },
  { status: "cancelled", color: "var(--color-data-neutral)" },
];
const ALL = "all";

export function CrawlerBand({ trends }: { trends?: Trends }) {
  const t = useAdminT();
  const locale = LOCALES[useUi().ui];
  const { overview: ov } = useLive();
  const [kind, setKind] = useState(ALL);
  const n = (v: number | null | undefined) => formatNumber(v, locale);
  const sum = (values: number[]) => values.reduce((a, v) => a + v, 0);
  const w = ov.worker;
  const days = trends?.days.map((d) => d.day);
  const runs = trends?.runs.filter((r) => kind === ALL || r.kind === kind);
  const count = (status: JobStatus, day?: string) =>
    sum(
      (runs ?? [])
        .filter((r) => r.status === status && (!day || r.day === day))
        .map((r) => r.n),
    );
  const failed = sum(
    (trends?.runs ?? []).filter((r) => r.status === "failed").map((r) => r.n),
  );
  const total = sum((trends?.runs ?? []).map((r) => r.n));
  const fetched = trends?.days.map((d) => d.fetched);

  return (
    <Band title={t("band.crawler")}>
      <Tiles>
        <MetricCard
          icon="admin:worker"
          label={t("meter.worker")}
          value={w.alive ? t("meter.workerOn") : t("meter.workerOff")}
          attention={w.alive ? undefined : "error"}
          sub={
            <>
              {t("meter.workerSub", { rss: formatBytes(w.rss_bytes, locale) })}{" "}
              {w.heartbeat_at ? (
                <Timestamp
                  value={w.heartbeat_at}
                  format="relative"
                  type="supporting"
                  color="secondary"
                />
              ) : (
                "—"
              )}
            </>
          }
        />
        <MetricCard
          icon="admin:running"
          label={t("tile.live")}
          value={n(ov.live_jobs.length)}
          sub={
            ov.live_jobs.length
              ? ov.live_jobs.map((j) => t(`kind.${j.kind}`)).join(" · ")
              : t("tile.liveIdle")
          }
        />
        <MetricCard
          icon="admin:booth"
          label={t("tile.booth")}
          value={n(w.booth_requests)}
          sub={t("tile.boothSub", {
            errors: n(w.booth_errors),
            limited: n(w.booth_rate_limited),
          })}
          attention={w.booth_rate_limited > 0 ? "warning" : undefined}
        />
        <MetricCard
          icon="admin:failed"
          label={t("tile.failed")}
          value={trends ? n(failed) : "—"}
          sub={t("tile.failedSub", { n: trends ? n(total) : "—" })}
          attention={failed > 0 ? "warning" : undefined}
        />
      </Tiles>
      <Charts>
        <BarChart
          title={t("chart.runs")}
          headline={
            runs && t("chart.total", { n: n(sum(runs.map((r) => r.n))) })
          }
          action={
            <Selector
              label={t("history.kind")}
              isLabelHidden
              size="sm"
              variant="secondary"
              value={kind}
              options={[
                { value: ALL, label: t("chart.allKinds") },
                ...KINDS.map((k) => ({ value: k, label: t(`kind.${k}`) })),
              ]}
              onChange={setKind}
            />
          }
          days={days}
          series={OUTCOMES.map(({ status, color }) => ({
            key: status,
            label: t(`status.${status}`),
            color,
            values: (days ?? []).map((day) => count(status, day)),
          }))}
          empty={t("chart.noRuns")}
        />
        <BarChart
          title={t("chart.fetched")}
          headline={fetched && t("chart.total", { n: n(sum(fetched)) })}
          days={days}
          series={[
            {
              key: "fetched",
              label: t("chart.fetched"),
              color: "var(--color-data-categorical-blue)",
              values: fetched ?? [],
            },
          ]}
          empty={t("chart.noRuns")}
        />
      </Charts>
    </Band>
  );
}
