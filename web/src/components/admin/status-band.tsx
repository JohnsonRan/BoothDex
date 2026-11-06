"use client";
// The overview's first band: whether the site is working and what waits on the operator. The worker, what runs
// now, the month's failed runs, Booth's answers since the worker started, the AI and image backlogs and the
// search index, with the index rebuild in the band heading. Tiles asking for attention come first, errors
// before warnings; the sort is stable, so the declared order breaks ties and a calm site keeps one order.

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { type ComponentProps, useState } from "react";
import { useUi } from "@/components/providers";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import { LOCALES } from "@/lib/settings-core";
import { useLive } from "./admin-console";
import { Band, Tiles } from "./band";
import { formatBytes, formatNumber } from "./format";
import { errorText, useAdminT } from "./messages";
import { MetricCard } from "./metric-card";
import type { Trends } from "./types";

const rank = (attention?: "warning" | "error") =>
  attention === "error" ? 0 : attention === "warning" ? 1 : 2;

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

export function StatusBand({ trends }: { trends?: Trends }) {
  const t = useAdminT();
  const toast = useToast();
  const locale = LOCALES[useUi().ui];
  const { overview: ov } = useLive();
  const [reloading, setReloading] = useState(false);
  const n = (v: number | null | undefined) => formatNumber(v, locale);
  const d = ov.data;
  const w = ov.worker;
  const stale = ov.catalog.version < ov.catalog.db_version;
  const runs = trends?.runs ?? [];
  const failed = runs
    .filter((r) => r.status === "failed")
    .reduce((a, r) => a + r.n, 0);
  const total = runs.reduce((a, r) => a + r.n, 0);

  const reload = () => {
    setReloading(true);
    api("/api/admin/reload", { method: "POST" })
      .then(
        () => toast({ body: t("toast.reload") }),
        (e) => toast({ type: "error", body: errorText(t, e) }),
      )
      .finally(() => setReloading(false));
  };

  const tiles: (ComponentProps<typeof MetricCard> & { id: string })[] = [
    {
      id: "worker",
      icon: "admin:worker",
      label: t("meter.worker"),
      value: w.alive ? t("meter.workerOn") : t("meter.workerOff"),
      attention: w.alive ? undefined : "error",
      sub: (
        <>
          {t("meter.workerSub", { rss: formatBytes(w.rss_bytes, locale) })}{" "}
          {ago(w.heartbeat_at)}
        </>
      ),
    },
    {
      id: "live",
      icon: "admin:running",
      label: t("tile.live"),
      value: n(ov.live_jobs.length),
      sub: ov.live_jobs.length
        ? ov.live_jobs.map((j) => t(`kind.${j.kind}`)).join(" · ")
        : t("tile.liveIdle"),
    },
    {
      id: "failed",
      icon: "admin:failed",
      label: t("tile.failed"),
      value: trends ? n(failed) : "—",
      sub: t("tile.failedSub", { n: trends ? n(total) : "—" }),
      attention: failed > 0 ? "warning" : undefined,
    },
    {
      id: "booth",
      icon: "admin:booth",
      label: t("tile.booth"),
      value: n(w.booth_requests),
      sub: t("tile.boothSub", {
        errors: n(w.booth_errors),
        limited: n(w.booth_rate_limited),
      }),
      attention: w.booth_rate_limited > 0 ? "warning" : undefined,
    },
    {
      id: "ai",
      icon: "admin:ai",
      label: t("tile.ai"),
      value: n(d.needs_enrichment),
      sub: t("tile.aiSub"),
      attention: d.needs_enrichment > 0 ? "warning" : undefined,
    },
    {
      id: "images",
      icon: "admin:images",
      label: t("tile.images"),
      value: n(d.missing_images),
      sub: t("tile.imagesSub"),
      attention: d.missing_images > 0 ? "warning" : undefined,
    },
    {
      id: "index",
      icon: "admin:index",
      label: t("tile.index"),
      value: n(ov.catalog.docs),
      attention: stale ? "warning" : undefined,
      sub: stale ? (
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
      ),
    },
  ];

  return (
    <Band
      title={t("band.status")}
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
        {tiles
          .sort((a, b) => rank(a.attention) - rank(b.attention))
          .map(({ id, ...tile }) => (
            <MetricCard key={id} {...tile} />
          ))}
      </Tiles>
    </Band>
  );
}
