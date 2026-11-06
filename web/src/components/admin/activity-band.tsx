"use client";
// The overview's activity band: the last 24 hours as tiles, then the daily trends that explain them. Finished
// crawler runs by outcome across the full width, one kind or all (picked as the template dashboard-alert-rail
// picks its scope: a Selector with a hidden label, page.tsx:1482-1489); under it items published on Booth and
// the item pages the details and refresh runs fetched, side by side. Outcome colours are categorical, not
// status, so they stay apart for colour-blind readers and a good run is never red; legend and tooltip name them.

import { Selector } from "@astryxdesign/core/Selector";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { LOCALES } from "@/lib/settings-core";
import { useLive } from "./admin-console";
import { Band, Charts, Tiles } from "./band";
import { formatNumber } from "./format";
import { useAdminT } from "./messages";
import { MetricCard } from "./metric-card";
import { TrendChart, type TrendSeries } from "./trend-chart";
import { type JobStatus, KINDS, type Trends } from "./types";

/** the first categorical colour: a lone series, and the runs that succeeded */
const FIRST = "var(--color-data-categorical-blue)";
const OUTCOMES: { status: JobStatus; color: string }[] = [
  { status: "succeeded", color: FIRST },
  { status: "failed", color: "var(--color-data-categorical-orange)" },
  { status: "cancelled", color: "var(--color-data-neutral)" },
];
const ALL = "all";

export function ActivityBand({ trends }: { trends?: Trends }) {
  const t = useAdminT();
  const locale = LOCALES[useUi().ui];
  const d = useLive().overview.data;
  const [kind, setKind] = useState(ALL);
  const n = (v: number | null | undefined) => formatNumber(v, locale);
  const days = trends?.days.map((x) => x.day);
  const runs = trends?.runs.filter((r) => kind === ALL || r.kind === kind);
  const count = (status: JobStatus, day: string) =>
    (runs ?? [])
      .filter((r) => r.status === status && r.day === day)
      .reduce((a, r) => a + r.n, 0);
  const daily = (key: "published" | "fetched"): TrendSeries[] => [
    {
      key,
      label: t(`chart.${key}`),
      color: FIRST,
      values: trends?.days.map((x) => x[key]) ?? [],
    },
  ];

  return (
    <Band title={t("band.activity")}>
      <Tiles>
        <MetricCard
          icon="admin:new"
          label={t("tile.new")}
          value={n(d.published_24h)}
          sub={t("tile.newSub")}
        />
        <MetricCard
          icon="admin:refreshed"
          label={t("tile.refreshed")}
          value={n(d.refreshed_24h)}
          sub={
            <>
              {t("tile.refreshedSub")}{" "}
              {d.stalest_scrape ? (
                <Timestamp
                  value={d.stalest_scrape}
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
      </Tiles>
      <TrendChart
        title={t("chart.runs")}
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
      <Charts>
        <TrendChart
          title={t("chart.published")}
          days={days}
          series={daily("published")}
          empty={t("chart.empty")}
        />
        <TrendChart
          title={t("chart.fetched")}
          days={days}
          series={daily("fetched")}
          empty={t("chart.noRuns")}
        />
      </Charts>
    </Band>
  );
}
