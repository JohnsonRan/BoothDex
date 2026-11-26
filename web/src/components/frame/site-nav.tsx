"use client";
// Primary view links in the desktop top bar (>= lg): All items, My favourites and Recently viewed, with live count
// badges once read in the browser so first paint matches server markup (hidden below lg in app/globals.css).

import { Badge } from "@astryxdesign/core/Badge";
import { Icon } from "@astryxdesign/core/Icon";
import { TopNavItem } from "@astryxdesign/core/TopNav";
import { usePathname } from "next/navigation";
import { useAccount } from "@/components/account/session";
import { useFilterUrl } from "@/components/catalog/filter/filter-url";
import { useUi } from "@/components/providers";
import type { Filters } from "@/lib/catalog";
import { prefersReducedMotion } from "@/lib/motion";
import { SIGNED_IN } from "@/lib/session-hint";
import { LOCALES } from "@/lib/settings-core";

/** Matches the recent view's single-page cap (`components/account/recent.tsx`, PAGE). */
export const RECENT_BADGE_CAP = 100;

/** Favourites view clears search, shop, avatar and category filters; shares behaviour with AccountMenu. */
export const FAVOURITES_PATCH: Partial<Filters> = {
  view: "favourites",
  avatar: "",
  q: "",
  author: "",
  sc: "",
  features: [],
};

/** Recent view keeps the picked avatar so returning to All items restores it; shares behaviour with AccountMenu. */
export const RECENT_PATCH: Partial<Filters> = {
  view: "recent",
  q: "",
  author: "",
  sc: "",
  features: [],
};

export type NavKey = "all" | "favourites" | "recent";

/** Active primary nav tab for the current route and view, or null on non-catalogue routes (`/privacy`, `/terms`). */
export function activeNavKey(
  pathname: string,
  view: Filters["view"],
): NavKey | null {
  if (pathname !== "/") return null;
  if (view === "favourites") return "favourites";
  if (view === "recent") return "recent";
  return "all";
}

const plainClick = (e: React.MouseEvent) =>
  !e.button && !e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey;

const scrollTop = () =>
  scrollTo({
    top: 0,
    behavior: prefersReducedMotion() ? "instant" : "smooth",
  });

function navBadge(count: string | null, selected: boolean) {
  return (
    count !== null && (
      <Badge
        label={count}
        style={{
          fontVariantNumeric: "tabular-nums",
          ...(selected
            ? { backgroundColor: "var(--color-background-surface)" }
            : undefined),
        }}
      />
    )
  );
}

export function SiteNav() {
  const { t, ui } = useUi();
  const pathname = usePathname();
  const { filters, apply } = useFilterUrl();
  const { me, favs, viewed, openSignIn } = useAccount();
  const active = activeNavKey(pathname, filters.view);
  const home = pathname === "/";
  const locale = LOCALES[ui];
  const favCount =
    me && favs && favs.size > 0 ? favs.size.toLocaleString(locale) : null;
  const recentCount =
    viewed && viewed.length > 0
      ? Math.min(viewed.length, RECENT_BADGE_CAP).toLocaleString(locale)
      : null;

  return (
    <>
      <TopNavItem
        label={t("title.all")}
        href="/"
        icon={<Icon icon="site:allAvatars" size="sm" />}
        isSelected={active === "all"}
        onClick={(e) => {
          if (!plainClick(e)) return;
          e.preventDefault();
          if (home && filters.view === "items") scrollTop();
          else apply({ view: "items" });
        }}
      />
      <TopNavItem
        label={t("title.favourites")}
        href="/?v=favourites"
        icon={<Icon icon="site:favourite" size="sm" />}
        isSelected={active === "favourites"}
        onClick={(e) => {
          if (!plainClick(e)) return;
          e.preventDefault();
          if (
            me === null ||
            (me === undefined &&
              document.documentElement.dataset.account !== SIGNED_IN)
          ) {
            openSignIn(
              home && filters.view === "favourites"
                ? undefined
                : () => apply(FAVOURITES_PATCH),
            );
          } else {
            if (home && filters.view === "favourites") scrollTop();
            else apply(FAVOURITES_PATCH);
          }
        }}
      >
        {t("title.favourites")}
        {navBadge(favCount, active === "favourites")}
      </TopNavItem>
      <TopNavItem
        label={t("title.recent")}
        href="/?v=recent"
        icon={<Icon icon="site:recent" size="sm" />}
        isSelected={active === "recent"}
        onClick={(e) => {
          if (!plainClick(e)) return;
          e.preventDefault();
          if (home && filters.view === "recent") scrollTop();
          else apply(RECENT_PATCH);
        }}
      >
        {t("title.recent")}
        {navBadge(recentCount, active === "recent")}
      </TopNavItem>
    </>
  );
}
