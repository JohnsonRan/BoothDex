"use client";
// The overview's catalogue band: what the site holds, the items with their listed and delisted split, the
// accounts with what they keep, and the database on disk.

import { useUi } from "@/components/providers";
import { LOCALES } from "@/lib/settings-core";
import { useLive } from "./admin-console";
import { Band, Tiles } from "./band";
import { formatBytes, formatNumber } from "./format";
import { useAdminT } from "./messages";
import { MetricCard } from "./metric-card";

export function CatalogueBand() {
  const t = useAdminT();
  const locale = LOCALES[useUi().ui];
  const { overview: ov } = useLive();
  const n = (v: number | null | undefined) => formatNumber(v, locale);
  const d = ov.data;
  return (
    <Band title={t("band.catalogue")}>
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
    </Band>
  );
}
