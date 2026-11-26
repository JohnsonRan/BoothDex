"use client";
// The one page frame every route renders in, adapted from the Astryx page template shell-top-nav
// (@astryxdesign/cli assets/templates/pages/shell-top-nav/page.tsx): AppShell + TopNav with the logo heading and
// primary view navigation (All items, My favourites, Recently viewed) in startContent, and the end actions on the
// right. Below lg (1024 px), AppShell moves the navigation links into its MobileNav drawer (seeded from server
// `mobile` so phones draw the mobile bar on first paint).
// End actions: the guestbook, the language and currency menus, the colour mode and the account (Settings and Sign in, or the
// account menu); the site search is the home page's filter row (catalog/search/search-box.tsx). Pages put their own
// Layout inside, so titles and content share the same left edge on every route.

import { AppShell } from "@astryxdesign/core/AppShell";
import { TopNav, TopNavHeading } from "@astryxdesign/core/TopNav";
import { usePathname, useRouter } from "next/navigation";
import { Activity, useState } from "react";
import { Account } from "@/components/account/account-button";
import { Favourites } from "@/components/account/favourites";
import { AccountProvider } from "@/components/account/session";
import { Guestbook } from "@/components/guestbook/guestbook-button";
import { ItemHost } from "@/components/item/item-host";
import { useUi } from "@/components/providers";
import { prefersReducedMotion } from "@/lib/motion";
import type { Rates } from "@/lib/settings-core";
import { BackToTop } from "./back-to-top";
import { ColorModeToggle, CurrencyMenu, LanguageMenu } from "./header-controls";
import { Logo } from "./logo";
import { SiteNav } from "./site-nav";

export function SiteFrame({
  rates,
  mobile = false,
  children,
}: {
  /** for the item dialog's prices */
  rates: Promise<Rates>;
  /** server hint so phones render the mobile bar on first paint without remounting TopNav */
  mobile?: boolean;
  children: React.ReactNode;
}) {
  const { t } = useUi();
  const router = useRouter();
  const pathname = usePathname();
  const [navOpen, setNavOpen] = useState(false);
  // On the home page the logo clears the filters and glides back to the top. Next scrolls a navigation only when
  // the page's first element is off screen (layout-router.js), which it is not here, and jumps when it does.
  const home = (e: React.MouseEvent) => {
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    setNavOpen(false);
    if (pathname !== "/") return;
    e.preventDefault();
    router.push("/", { scroll: false });
    scrollTo({
      top: 0,
      behavior: prefersReducedMotion() ? "instant" : "smooth",
    });
  };
  return (
    <AccountProvider>
      {/* the item dialog (`?item=`) opens from tiles and the dialog itself */}
      <ItemHost rates={rates}>
        {/* auto height: the window scrolls (and a phone's address bar collapses); the nav stays sticky */}
        <AppShell
          variant="surface"
          height="auto"
          mobileNav={{
            breakpoint: "lg",
            defaultIsMobile: mobile,
            isOpen: navOpen,
            onOpenChange: setNavOpen,
          }}
          topNav={
            <TopNav
              label={t("nav.main")}
              // one frame inset (TEMPLATE-MAP rule 5): the bar's own 8 px plus this put the logo on the content's
              // 24 px edge and the last (borderless) action's label within its padding of the right edge
              style={{ paddingInline: "var(--spacing-4)" }}
              heading={
                <TopNavHeading
                  // the wordmark is visually hidden on phones in the top bar (globals.css), as on master; the link keeps its name
                  className="site-heading"
                  heading="BoothDex"
                  headingHref="/"
                  logo={<Logo />}
                  onClick={home}
                />
              }
              startContent={<SiteNav onNavigate={() => setNavOpen(false)} />}
              endContent={
                <>
                  <Guestbook />
                  <LanguageMenu />
                  <CurrencyMenu />
                  <ColorModeToggle />
                  <Account />
                </>
              }
            />
          }
        >
          {/* hydrated on its own (a visible Activity is a hydration boundary, streamed inline unlike a Suspense
              boundary): the bar responds once its own code is in, without waiting for the page's */}
          <Activity>{children}</Activity>
        </AppShell>
      </ItemHost>
      <Favourites />
      <BackToTop />
    </AccountProvider>
  );
}
