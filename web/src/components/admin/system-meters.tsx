"use client";
// The overview's system section: host CPU, memory and disk, the image cache and the worker, each a MetricCard
// with a share bar where there is one.

import { Grid } from "@astryxdesign/core/Grid";
import { Text } from "@astryxdesign/core/Text";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { useUi } from "@/components/providers";
import { LOCALES } from "@/lib/settings-core";
import { useLive } from "./admin-console";
import { formatBytes, formatNumber } from "./format";
import { useAdminT } from "./messages";
import { MetricCard } from "./metric-card";

const pct = (v: number) => `${Math.round(v)}%`;

export function SystemMeters() {
  const t = useAdminT();
  const locale = LOCALES[useUi().ui];
  const { overview } = useLive();
  const { system: s, imgproxy: img, worker: w } = overview;
  const bytes = (v: number | null | undefined) => formatBytes(v, locale);
  const ts = (v: string) => (
    <Timestamp
      value={v}
      format="relative"
      type="supporting"
      color="secondary"
    />
  );
  const disk =
    s?.disk_total && s.disk_free != null
      ? ((s.disk_total - s.disk_free) / s.disk_total) * 100
      : undefined;
  return (
    <Grid columns={{ minWidth: 240, repeat: "fit" }} gap={4}>
      {s ? (
        <>
          <MetricCard
            label={t("meter.cpu")}
            value={pct(s.cpu_percent)}
            percent={s.cpu_percent}
            sub={t("meter.cpuSub", {
              cores: s.cpu_count,
              load: s.load_avg?.toFixed(2) ?? "—",
              api: s.api_cpu_percent?.toFixed(0) ?? "—",
            })}
          />
          <MetricCard
            label={t("meter.mem")}
            value={pct((s.mem_used / s.mem_total) * 100)}
            percent={(s.mem_used / s.mem_total) * 100}
            sub={t("meter.memSub", {
              used: bytes(s.mem_used),
              total: bytes(s.mem_total),
              rss: bytes(s.api_rss),
            })}
          />
          <MetricCard
            label={t("meter.disk")}
            value={disk === undefined ? "—" : pct(disk)}
            percent={disk}
            sub={t("meter.diskSub", {
              free: bytes(s.disk_free),
              total: bytes(s.disk_total),
              mount: s.disk_mount ?? "—",
            })}
          />
        </>
      ) : (
        <Text color="secondary">{t("system.unavailable")}</Text>
      )}
      {img?.reachable ? (
        <MetricCard
          label={t("meter.cache")}
          value={pct((img.hit_rate ?? 0) * 100)}
          percent={(img.hit_rate ?? 0) * 100}
          sub={t("meter.cacheSub", {
            requests: formatNumber(img.requests, locale),
            out: bytes(img.bytes_out),
            errors: formatNumber(img.errors, locale),
            disk: formatNumber(img.disk_cache_mb, locale),
            mem: formatNumber(img.memory_cache_mb, locale),
          })}
        />
      ) : (
        <MetricCard
          label={t("meter.cache")}
          value="—"
          isWarning
          sub={t("meter.cacheDown")}
        />
      )}
      <MetricCard
        label={t("meter.worker")}
        value={w.alive ? t("worker.on") : t("worker.off")}
        isWarning={!w.alive}
        sub={
          <>
            {t("meter.workerSub", { rss: bytes(w.rss_bytes) })}{" "}
            {w.started_at ? ts(w.started_at) : "—"} · {t("meter.heartbeat")}{" "}
            {w.heartbeat_at ? ts(w.heartbeat_at) : "—"}
          </>
        }
      />
    </Grid>
  );
}
