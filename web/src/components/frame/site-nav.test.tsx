import { expect, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import { PathnameContext } from "next/dist/shared/lib/hooks-client-context.shared-runtime";
import { renderToStaticMarkup } from "react-dom/server";
import { AccountProvider } from "@/components/account/session";
import { FilterUrlProvider } from "@/components/catalog/filter/filter-url";
import { Providers } from "@/components/providers";
import { DEFAULT_FILTERS, type Filters } from "@/lib/catalog";
import { browserMessages } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { activeNavKey, SiteNav } from "./site-nav";

const renderNav = (pathname: string, view: Filters["view"] = "items") =>
  renderToStaticMarkup(
    <AppRouterContext value={{} as AppRouterInstance}>
      <PathnameContext value={pathname}>
        <Providers settings={DEFAULT_SETTINGS} messages={browserMessages("en")}>
          <AccountProvider>
            <FilterUrlProvider filters={{ ...DEFAULT_FILTERS, view }}>
              <SiteNav />
            </FilterUrlProvider>
          </AccountProvider>
        </Providers>
      </PathnameContext>
    </AppRouterContext>,
  );

test("activeNavKey matches catalogue views and clears on non-catalogue routes", () => {
  expect(activeNavKey("/", "items")).toBe("all");
  expect(activeNavKey("/", "avatars")).toBe("all");
  expect(activeNavKey("/", "favourites")).toBe("favourites");
  expect(activeNavKey("/", "recent")).toBe("recent");
  expect(activeNavKey("/privacy", "items")).toBeNull();
  expect(activeNavKey("/terms", "favourites")).toBeNull();
});

test("SiteNav renders the three view links with SSR selection and no pre-load badges", () => {
  const cases: {
    path: string;
    view: Filters["view"];
    currentHref: string | null;
  }[] = [
    { path: "/", view: "items", currentHref: "/" },
    { path: "/", view: "avatars", currentHref: "/" },
    { path: "/", view: "favourites", currentHref: "/?v=favourites" },
    { path: "/", view: "recent", currentHref: "/?v=recent" },
    { path: "/privacy", view: "items", currentHref: null },
  ];

  for (const { path, view, currentHref } of cases) {
    const html = renderNav(path, view);
    expect(html).toContain('href="/"');
    expect(html).toContain('href="/?v=favourites"');
    expect(html).toContain('href="/?v=recent"');
    expect(html).not.toContain("astryx-badge");
    const links = [...html.matchAll(/<a [^>]*>/g)].map((m) => m[0]);
    expect(links).toHaveLength(3);
    const activeLinks = links.filter((tag) =>
      tag.includes('aria-current="page"'),
    );
    if (currentHref) {
      expect(activeLinks).toHaveLength(1);
      expect(activeLinks[0]).toContain(`href="${currentHref}"`);
    } else {
      expect(activeLinks).toHaveLength(0);
    }
  }
});
