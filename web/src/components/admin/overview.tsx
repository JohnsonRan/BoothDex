"use client";
// The overview, from the Astryx template dashboard-scorecard (templates/pages/dashboard-scorecard/page.tsx:
// 752-874): titled bands 40 px apart (:808), each a tight grid of number tiles. The bands run in the order an
// operator reads them: status first (what is broken or waiting, sorted to the front), then the last 24 hours
// with the daily trends under them, then what the catalogue holds, then the host. Tiles are live over the admin
// stream; the trends come from their own endpoint.

import { VStack } from "@astryxdesign/core/Layout";
import { ActivityBand } from "./activity-band";
import { CatalogueBand } from "./catalogue-band";
import { LiveStatus } from "./live-status";
import { useAdminT } from "./messages";
import { PageFrame } from "./page-frame";
import { StatusBand } from "./status-band";
import { SystemBand } from "./system-band";
import { useTrends } from "./use-trends";

function Bands() {
  const trends = useTrends();
  return (
    <VStack gap={10}>
      <StatusBand trends={trends} />
      <ActivityBand trends={trends} />
      <CatalogueBand />
      <SystemBand />
    </VStack>
  );
}

export function Overview() {
  const t = useAdminT();
  return (
    <PageFrame title={t("nav.overview")} meta={<LiveStatus />}>
      <Bands />
    </PageFrame>
  );
}
