"use client";
// The one page frame every route renders in (Top Nav template: AppShell + TopNav). Pages put their own
// Layout inside, so titles and content share the same left edge on every route. A client module: the logo is
// an icon component, which cannot cross from the server as a prop.

import { AppShell } from "@astryxdesign/core/AppShell";
import { Icon } from "@astryxdesign/core/Icon";
import { NavIcon } from "@astryxdesign/core/NavIcon";
import { TopNav, TopNavHeading } from "@astryxdesign/core/TopNav";
import { PackageSearch } from "lucide-react";

export function SiteFrame({
  label,
  children,
}: {
  /** accessible name of the navigation landmark */
  label: string;
  children: React.ReactNode;
}) {
  return (
    // auto height: the window scrolls (and a phone's address bar collapses); the nav stays sticky
    <AppShell
      variant="surface"
      height="auto"
      topNav={
        <TopNav
          label={label}
          heading={
            <TopNavHeading
              heading="BoothDex"
              headingHref="/"
              logo={<NavIcon icon={<Icon icon={PackageSearch} size="sm" />} />}
            />
          }
        />
      }
    >
      {children}
    </AppShell>
  );
}
