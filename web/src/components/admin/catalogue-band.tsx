"use client";
// The overview's catalogue band: what the catalogue holds and what still waits on the crawler or the AI, the
// search index with its rebuild action in the band heading, then items published on Booth per day.

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { useToast } from "@astryxdesign/core/Toast";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import { LOCALES } from "@/lib/settings-core";
import { useLive } from "./admin-console";
import { Band, Charts, Tiles } from "./band";
import { BarChart } from "./bar-chart";
import { formatBytes, formatNumber } from "./format";
import { errorText, useAdminT } from "./messages";
import { MetricCard } from "./metric-card";
import type { Trends } from "./types";

const ago = (v: string | null) =>
  v ? (
    <Timestamp
      value={v}
      format="relative"
      type="supporting"
      color="secondary"
    />
  ) : (
    "—"
  );

export function CatalogueBand({ trends }: { trends?: Trends }) {
  const t = useAdminT();
  const toast = useToast();
  const locale = LOCALES[useUi().ui];
  const { overview: ov } = useLive();
  const [reloading, setReloading] = useState(false);
  const n = (v: number | null | undefined) => formatNumber(v, locale);
  const d = ov.data;
  const stale = ov.catalog.version < ov.catalog.db_version;
  const published = trends?.days.map((x) => x.published);

  const reload = () => {
    setReloading(true);
    api("/api/admin/reload", { method: "POST" })
      .then(
        () => toast({ body: t("toast.reload") }),
        (e) => toast({ type: "error", body: errorText(t, e) }),
      )
      .finally(() => setReloading(false));
  };

  return (
    <Band
      title={t("band.catalogue")}
      action={
        <Button
          label={t("tile.reload")}
          variant="secondary"
          icon={<Icon icon="admin:reload" size="sm" />}
          isLoading={reloading}
          onClick={reload}
        />
      }
    >
      <Tiles>
        <MetricCard
          icon="admin:items"
          label={t("tile.items")}
          value={n(d.items)}
          sub={t("tile.itemsSub", {
            listed: n(d.listed),
            delisted: n(d.delisted),
          })}
        />
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
              {t("tile.refreshedSub")} {ago(d.stalest_scrape)}
            </>
          }
        />
        <MetricCard
          icon="admin:ai"
          label={t("tile.ai")}
          value={n(d.needs_enrichment)}
          sub={t("tile.aiSub")}
          attention={d.needs_enrichment > 0 ? "warning" : undefined}
        />
        <MetricCard
          icon="admin:images"
          label={t("tile.images")}
          value={n(d.missing_images)}
          sub={t("tile.imagesSub")}
          attention={d.missing_images > 0 ? "warning" : undefined}
        />
        <MetricCard
          icon="admin:index"
          label={t("tile.index")}
          value={n(ov.catalog.docs)}
          attention={stale ? "warning" : undefined}
          sub={
            stale ? (
              t("tile.indexStale")
            ) : (
              <>
                {t("tile.indexSub", {
                  v: ov.catalog.version,
                  secs: ov.catalog.build_secs.toLocaleString(locale, {
                    maximumFractionDigits: 1,
                  }),
                })}{" "}
                {ago(ov.catalog.loaded_at)}
              </>
            )
          }
        />
        <MetricCard
          icon="admin:users"
          label={t("tile.users")}
          value={n(d.users)}
          sub={t("tile.usersSub", {
            fav: n(d.favourites),
            saved: n(d.saved_searches),
          })}
        />
        <MetricCard
          icon="admin:db"
          label={t("tile.db")}
          value={formatBytes(d.db_bytes, locale)}
          sub={t("tile.dbSub", {
            size: formatBytes(ov.catalog.bitmap_bytes, locale),
          })}
        />
      </Tiles>
      <Charts>
        <BarChart
          title={t("chart.published")}
          headline={
            published &&
            t("chart.total", { n: n(published.reduce((a, v) => a + v, 0)) })
          }
          days={trends?.days.map((x) => x.day)}
          series={[
            {
              key: "published",
              label: t("chart.published"),
              color: "var(--color-data-categorical-blue)",
              values: published ?? [],
            },
          ]}
          empty={t("chart.empty")}
        />
      </Charts>
    </Band>
  );
}
