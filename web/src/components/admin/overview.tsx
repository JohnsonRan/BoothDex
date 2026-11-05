"use client";
// The overview, from the Astryx page template dashboard (templates/pages/dashboard/page.tsx): its MetricCard
// (:586-623, label, large value, supporting line) in a fit grid (:746-764), a Divider, then a titled section
// (:766-776; heading levels as table-page :577) of system meters, which use the template's ProgressBar (:369-375). Live over the admin stream.

import { Button } from "@astryxdesign/core/Button";
import { Divider } from "@astryxdesign/core/Divider";
import { Grid } from "@astryxdesign/core/Grid";
import { Icon } from "@astryxdesign/core/Icon";
import { VStack } from "@astryxdesign/core/Layout";
import { Heading } from "@astryxdesign/core/Text";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { useToast } from "@astryxdesign/core/Toast";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import { LOCALES } from "@/lib/settings-core";
import { useLive } from "./admin-console";
import { formatBytes, formatNumber } from "./format";
import { LiveStatus } from "./live-status";
import { useAdminT } from "./messages";
import { MetricCard } from "./metric-card";
import { PageFrame } from "./page-frame";
import { SystemMeters } from "./system-meters";

const GRID = { minWidth: 240, repeat: "fit" } as const;

export function Overview() {
  const t = useAdminT();
  const toast = useToast();
  const locale = LOCALES[useUi().ui];
  const { overview: ov } = useLive();
  const [reloading, setReloading] = useState(false);
  const d = ov.data;
  const n = (v: number | null | undefined) => formatNumber(v, locale);
  const stale = ov.catalog.version < ov.catalog.db_version;
  const w = ov.worker;

  const reload = () => {
    setReloading(true);
    api("/api/admin/reload", { method: "POST" })
      .then(
        () => toast({ body: t("toast.reload") }),
        (e: Error) => toast({ type: "error", body: e.message }),
      )
      .finally(() => setReloading(false));
  };

  return (
    <PageFrame title={t("nav.overview")} meta={<LiveStatus />}>
      <Grid columns={GRID} gap={4}>
        <MetricCard
          label={t("tile.items")}
          value={n(d.items)}
          sub={t("tile.itemsSub", {
            listed: n(d.listed),
            delisted: n(d.delisted),
          })}
        />
        <MetricCard
          label={t("tile.new")}
          value={n(d.published_24h)}
          sub={t("tile.newSub", { n: n(d.refreshed_24h) })}
        />
        <MetricCard
          label={t("tile.ai")}
          value={n(d.needs_enrichment)}
          sub={t("tile.aiSub")}
          isWarning={d.needs_enrichment > 0}
        />
        <MetricCard
          label={t("tile.images")}
          value={n(d.missing_images)}
          sub={
            <>
              {t("tile.imagesSub")}{" "}
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
          isWarning={d.missing_images > 0}
        />
        <MetricCard
          label={t("tile.users")}
          value={n(d.users)}
          sub={t("tile.usersSub", {
            fav: n(d.favourites),
            saved: n(d.saved_searches),
          })}
        />
        <MetricCard
          label={t("tile.index")}
          value={`v${ov.catalog.version}`}
          isWarning={stale}
          sub={
            stale ? (
              t("tile.indexStale")
            ) : (
              <>
                {t("tile.indexSub", {
                  docs: n(ov.catalog.docs),
                  secs: ov.catalog.build_secs.toLocaleString(locale, {
                    maximumFractionDigits: 1,
                  }),
                })}{" "}
                <Timestamp
                  value={ov.catalog.loaded_at}
                  format="relative"
                  type="supporting"
                  color="secondary"
                />
              </>
            )
          }
          action={
            <Button
              label={t("tile.reload")}
              variant="secondary"
              size="sm"
              icon={<Icon icon="admin:reload" size="sm" />}
              isLoading={reloading}
              onClick={reload}
            />
          }
        />
        <MetricCard
          label={t("tile.db")}
          value={formatBytes(d.db_bytes, locale)}
          sub={t("tile.dbSub", {
            size: formatBytes(ov.catalog.bitmap_bytes, locale),
          })}
        />
        <MetricCard
          label={t("tile.booth")}
          value={n(w.booth_requests)}
          sub={t("tile.boothSub", {
            errors: n(w.booth_errors),
            limited: n(w.booth_rate_limited),
          })}
          isWarning={w.booth_rate_limited > 0}
        />
      </Grid>
      <Divider />
      <VStack gap={4}>
        <Heading level={3} accessibilityLevel={2}>
          {t("system.title")}
        </Heading>
        <SystemMeters />
      </VStack>
    </PageFrame>
  );
}
