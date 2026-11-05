"use client";
// The one page frame every route renders in, adapted from the Astryx page template shell-top-nav
// (templates/pages/shell-top-nav/page.tsx): AppShell + TopNav with the logo heading on the left and the end
// actions on the right. BoothDex has no section navigation, so the template's centre mega menus are not used.
// End actions: the magnifier opens the quick-search palette (Ctrl/Cmd+K too, as in template shell-nav), then
// the language and currency menus, the colour mode and Sign in. Pages put their own Layout inside, so titles
// and content share the same left edge on every route.

import { AppShell } from "@astryxdesign/core/AppShell";
import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { TopNav, TopNavHeading } from "@astryxdesign/core/TopNav";
import dynamic from "next/dynamic";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
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
    <AppShell
      variant="surface"
      height="auto"
      topNav={
        <TopNav
          label={t("nav.main")}
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
              {/* the sign-in dialog is the next step of the rewrite; the button is shown but opens nothing yet */}
              <Button label={t("nav.signIn")} variant="ghost" />
            </>
          }
        />
      }
    >
      {children}
    </AppShell>
  );
}

function QuickSearch({ rates }: { rates: Promise<Rates> }) {
  const { t } = useUi();
  const router = useRouter();
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
    else if (p.kind === "item") {
      // the item dialog reads ?item= (next step of the rewrite)
      const q = new URLSearchParams(location.search);
      q.set("item", String(p.value));
      router.push(`?${q}`, { scroll: false });
    }
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
