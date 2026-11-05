"use client";
// The overview, from the Astryx template dashboard-scorecard (templates/pages/dashboard-scorecard/page.tsx:
// 752-874): titled bands 40 px apart (:808), each a tight grid of number tiles, with the daily trend under the
// tiles it explains, as the reference admin puts its trend under the "today" band. Catalogue, crawler,
// system. Tiles are live over the admin stream; the charts come from the trends endpoint.

import { VStack } from "@astryxdesign/core/Layout";
import { CatalogueBand } from "./catalogue-band";
import { CrawlerBand } from "./crawler-band";
import { LiveStatus } from "./live-status";
import { useAdminT } from "./messages";
import { PageFrame } from "./page-frame";
import { SystemBand } from "./system-band";
import { useTrends } from "./use-trends";

export function Overview() {
  const t = useAdminT();
  const trends = useTrends();
  return (
    <PageFrame title={t("nav.overview")} meta={<LiveStatus />}>
      <VStack gap={10}>
        <CatalogueBand trends={trends} />
        <CrawlerBand trends={trends} />
        <SystemBand />
      </VStack>
    </PageFrame>
  );
}
