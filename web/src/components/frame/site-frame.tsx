"use client";
// The one page frame every route renders in, adapted from the Astryx page template shell-top-nav
// (templates/pages/shell-top-nav/page.tsx): AppShell + TopNav with the logo heading on the left and the end
// actions on the right. BoothDex has no section navigation, so the template's centre mega menus are not used.
// End actions: the language and currency menus, the colour mode and the account (Settings and Sign in, or the
// account menu); the site search is the home page's filter row (catalog/search-box.tsx). Pages put their own
// Layout inside, so titles and content share the same left edge on every route.

import { AppShell } from "@astryxdesign/core/AppShell";
import { TopNav, TopNavHeading } from "@astryxdesign/core/TopNav";
import { Account } from "@/components/account/account-button";
import { Favourites } from "@/components/account/favourites";
import { AccountProvider } from "@/components/account/session";
import { ItemHost } from "@/components/item/item-host";
import { useUi } from "@/components/providers";
import type { Rates } from "@/lib/settings-core";
import { ColorModeToggle, CurrencyMenu, LanguageMenu } from "./header-controls";
import { Logo } from "./logo";

export function SiteFrame({
  rates,
  children,
}: {
  /** for the item dialog's prices */
  rates: Promise<Rates>;
  children: React.ReactNode;
}) {
  const { t } = useUi();
  return (
    <AccountProvider>
      {/* the item dialog (`?item=`) opens from tiles and the dialog itself */}
      <ItemHost rates={rates}>
        {/* auto height: the window scrolls (and a phone's address bar collapses); the nav stays sticky */}
        <AppShell
          variant="surface"
          height="auto"
          // no drawer: the bar is the same on every width. Astryx's phone bar is chosen only in the browser, so
          // it replaced the server's end actions after hydration and lost the clicks made before that
          mobileNav={false}
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
      </ItemHost>
      <Favourites />
    </AccountProvider>
  );
}
