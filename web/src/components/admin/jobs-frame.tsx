"use client";
// The crawler's two pages share one title with a tab row of links (Tasks, History) in the header, as the
// Astryx template detail-page lays out its tabs (@astryxdesign/cli assets/templates/pages/detail-page/page.tsx:252-260: size lg, full bleed).

import { Tab, TabList } from "@astryxdesign/core/TabList";
import { useAdminT } from "./messages";
import { PageFrame } from "./page-frame";

const TABS = { tasks: "/admin/jobs", history: "/admin/jobs/history" } as const;

export function JobsFrame({
  tab,
  children,
}: {
  tab: keyof typeof TABS;
  children: React.ReactNode;
}) {
  const t = useAdminT();
  return (
    <PageFrame
      title={t("nav.jobs")}
      tabs={
        // the tabs are links: navigation selects them, so there is no state to change
        <TabList value={TABS[tab]} onChange={() => {}} size="lg" isFullBleed>
          <Tab value={TABS.tasks} href={TABS.tasks} label={t("jobs.tasks")} />
          <Tab
            value={TABS.history}
            href={TABS.history}
            label={t("jobs.history")}
          />
        </TabList>
      }
    >
      {children}
    </PageFrame>
  );
}
