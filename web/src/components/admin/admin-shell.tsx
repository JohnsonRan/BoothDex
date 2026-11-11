"use client";
// The admin frame, from the Astryx page template shell-side-nav (templates/pages/shell-side-nav/page.tsx:164-201):
// AppShell with a collapsible, resizable SideNav, its heading on top, the menu sections, and an account section
// in the footer. The template's conversation list becomes two titled groups (Operate, System); the footer holds
// the way back to the site and the site's own language and colour-mode controls. On phones AppShell turns the
// nav into a drawer behind its own top bar; the server renders that phone shell when the request comes from a
// phone, so the first paint does not show the desktop nav first.

import { AppShell } from "@astryxdesign/core/AppShell";
import {
  SideNav,
  SideNavHeading,
  SideNavItem,
  SideNavSection,
} from "@astryxdesign/core/SideNav";
import { usePathname } from "next/navigation";
import {
  ColorModeToggle,
  LanguageMenu,
} from "@/components/frame/header-controls";
import { Logo } from "@/components/frame/logo";
import { type AdminKey, useAdminT } from "./messages";
import "./icons";

type NavLink = { href: string; label: AdminKey; icon: `admin:${string}` };
const GROUPS: { title: AdminKey; links: NavLink[] }[] = [
  {
    title: "nav.operate",
    links: [
      { href: "/admin", label: "nav.overview", icon: "admin:overview" },
      { href: "/admin/jobs", label: "nav.jobs", icon: "admin:jobs" },
      { href: "/admin/users", label: "nav.users", icon: "admin:users" },
    ],
  },
  {
    title: "nav.system",
    links: [
      {
        href: "/admin/settings",
        label: "nav.settings",
        icon: "admin:settings",
      },
    ],
  },
];

/** Whole-segment prefix match, so /admin/jobs/history keeps Crawler selected and /admin selects only itself. */
const isCurrent = (path: string, href: string) =>
  href === "/admin"
    ? path === href
    : path === href || path.startsWith(`${href}/`);

export function AdminShell({
  mobile,
  children,
}: {
  mobile: boolean;
  children: React.ReactNode;
}) {
  const t = useAdminT();
  const path = usePathname();
  return (
    <AppShell
      contentPadding={0}
      mobileNav={{ defaultIsMobile: mobile }}
      sideNav={
        <SideNav
          collapsible
          resizable={{ defaultWidth: 260, minWidth: 220, maxWidth: 360 }}
          header={
            <SideNavHeading
              heading={t("title")}
              // the mark itself, as in the site's top bar; NavIcon would paint an accent disc behind it
              icon={<Logo />}
              headingHref="/admin"
            />
          }
          footer={
            <SideNavSection title={t("nav.site")} isHeaderHidden>
              <SideNavItem label={t("nav.site")} icon="admin:site" href="/" />
            </SideNavSection>
          }
          footerIcons={
            <>
              <LanguageMenu />
              <ColorModeToggle />
            </>
          }
        >
          {GROUPS.map((g) => (
            <SideNavSection key={g.title} title={t(g.title)}>
              {g.links.map((l) => (
                <SideNavItem
                  key={l.href}
                  label={t(l.label)}
                  icon={l.icon}
                  href={l.href}
                  isSelected={isCurrent(path, l.href)}
                />
              ))}
            </SideNavSection>
          ))}
        </SideNav>
      }
    >
      {children}
    </AppShell>
  );
}
