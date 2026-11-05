"use client";
// The one page frame every route renders in, adapted from the Astryx page template shell-top-nav
// (templates/pages/shell-top-nav/page.tsx): AppShell + TopNav with the logo heading on the left and the end
// actions on the right. BoothDex has no section navigation, so the template's centre mega menus are not used.
// End actions: the magnifier opens the quick-search palette (Ctrl/Cmd+K too, as in template shell-nav), then
// the language and currency menus, the colour mode and the account (Settings and Sign in, or the account menu). Pages put their own Layout inside, so titles
// and content share the same left edge on every route.

import { AppShell } from "@astryxdesign/core/AppShell";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { TopNav, TopNavHeading } from "@astryxdesign/core/TopNav";
import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import { Account } from "@/components/account/account-button";
import { AccountProvider } from "@/components/account/session";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import type { Rates } from "@/lib/settings-core";
import { ColorModeToggle, CurrencyMenu, LanguageMenu } from "./header-controls";
import { Logo } from "./logo";
import type { Pick } from "./palette-source";

// the palette's code loads on first open, not with the page
const Palette = dynamic(() => import("./palette"), { ssr: false });

export function SiteFrame({
  rates,
  children,
}: {
  /** for the palette's item prices */
  rates: Promise<Rates>;
  children: React.ReactNode;
}) {
  const { t } = useUi();
  return (
    // auto height: the window scrolls (and a phone's address bar collapses); the nav stays sticky
    <AccountProvider>
      <AppShell
        variant="surface"
        height="auto"
        topNav={
          <TopNav
            label={t("nav.main")}
            // one frame inset (TEMPLATE-MAP rule 5): the bar's own 8 px plus this put the logo on the content's
            // 24 px edge and the last (borderless) action's label within its padding of the right edge
            style={{ paddingInline: "var(--spacing-4)" }}
            heading={
              <TopNavHeading
                // the wordmark is visually hidden on phones (globals.css), as on master; the link keeps its name
                className="site-heading"
                heading="BoothDex"
                headingHref="/"
                logo={<Logo />}
              />
            }
            endContent={
              <>
                <QuickSearch rates={rates} />
                <LanguageMenu />
                <CurrencyMenu />
                <ColorModeToggle />
                <Account />
              </>
            }
          />
        }
      >
        {children}
      </AppShell>
    </AccountProvider>
  );
}

function QuickSearch({ rates }: { rates: Promise<Rates> }) {
  const { t } = useUi();
  const { filters, apply } = useFilterUrl();
  // undefined: never opened, so the palette's code is not loaded
  const [open, setOpen] = useState<boolean>();
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    addEventListener("keydown", key);
    return () => removeEventListener("keydown", key);
  }, []);
  const pick = (p: Pick) => {
    if (p.kind === "query") apply({ q: p.value });
    else if (p.kind === "avatar") apply({ avatar: p.value });
    else if (p.kind === "shop") apply({ author: p.value, q: "", avatar: "" });
    // the item dialog is the next step of the rewrite; until then the item opens on Booth
    else if (p.kind === "item")
      window.open(`https://booth.pm/ja/items/${p.value}`, "_blank", "noopener");
  };
  return (
    <>
      <IconButton
        label={t("nav.quickSearch")}
        tooltip={`${t("nav.quickSearch")} (Ctrl K)`}
        variant="ghost"
        icon={<Icon icon="search" size="sm" />}
        onClick={() => setOpen(true)}
      />
      {open !== undefined && (
        <Palette
          isOpen={open}
          onOpenChange={setOpen}
          adult={filters.adult}
          rates={rates}
          onPick={pick}
        />
      )}
    </>
  );
}
