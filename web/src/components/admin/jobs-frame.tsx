"use client";
// The crawler's two pages share one title with an underline tab row of links (Tasks, History), as the reference
// admin puts sub-pages under their title.

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
        <TabList value={TABS[tab]} onChange={() => {}}>
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
