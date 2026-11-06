"use client";
// The overview's system band: the API host's CPU, memory and disk as usage tiles (the bar warns from 80 %), and
// the image cache's hit rate, where higher is better, so it gets no bar and no warning colour.

import { Text } from "@astryxdesign/core/Text";
import { useUi } from "@/components/providers";
import { LOCALES } from "@/lib/settings-core";
import { useLive } from "./admin-console";
import { Band, Tiles } from "./band";
import { formatBytes, formatNumber } from "./format";
import { useAdminT } from "./messages";
import { MetricCard } from "./metric-card";

const pct = (v: number) => `${Math.round(v)}%`;

export function SystemBand() {
  const t = useAdminT();
  const locale = LOCALES[useUi().ui];
  const { system: s, imgproxy: img } = useLive().overview;
  const bytes = (v: number | null | undefined) => formatBytes(v, locale);
  const mem = s && (s.mem_used / s.mem_total) * 100;
  const disk =
    s?.disk_total && s.disk_free != null
      ? ((s.disk_total - s.disk_free) / s.disk_total) * 100
      : undefined;
  return (
    <Band title={t("band.system")}>
      {!s && <Text color="secondary">{t("system.unavailable")}</Text>}
      <Tiles>
        {s && mem != null && (
          <>
            <MetricCard
              icon="admin:cpu"
              label={t("meter.cpu")}
              value={pct(s.cpu_percent)}
              usage={s.cpu_percent}
              sub={t("meter.cpuSub", {
                cores: s.cpu_count,
                load: s.load_avg?.toFixed(2) ?? "—",
                api: s.api_cpu_percent?.toFixed(0) ?? "—",
              })}
            />
            <MetricCard
              icon="admin:memory"
              label={t("meter.mem")}
              value={pct(mem)}
              usage={mem}
              sub={t("meter.memSub", {
                used: bytes(s.mem_used),
                total: bytes(s.mem_total),
                rss: bytes(s.api_rss),
              })}
            />
            <MetricCard
              icon="admin:disk"
              label={t("meter.disk")}
              value={disk === undefined ? "—" : pct(disk)}
              usage={disk}
              sub={t("meter.diskSub", {
                free: bytes(s.disk_free),
                total: bytes(s.disk_total),
                mount: s.disk_mount ?? "—",
              })}
            />
          </>
        )}
        <MetricCard
          icon="admin:cache"
          label={t("meter.cache")}
          value={img?.reachable ? pct((img.hit_rate ?? 0) * 100) : "—"}
          attention={img?.reachable ? undefined : "error"}
          sub={
            img?.reachable
              ? t("meter.cacheSub", {
                  requests: formatNumber(img.requests, locale),
                  errors: formatNumber(img.errors, locale),
                  disk: formatNumber(img.disk_cache_mb, locale),
                })
              : t("meter.cacheDown")
          }
        />
      </Tiles>
    </Band>
  );
}
