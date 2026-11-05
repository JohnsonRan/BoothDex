"use client";
// Filter state lives in the URL. The server parses it and renders the results; changing a filter navigates,
// and the new results arrive as a server render inside a transition, so the old ones stay until then. Whether
// one is under way has its own context (useRefresh), so the controls do not render again when it starts or ends.

import { usePathname, useRouter } from "next/navigation";
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useTransition,
} from "react";
import { DEFAULT_FILTERS, type Filters, filtersToQuery } from "@/lib/catalog";
import { addQuery, loadHistory, saveHistory } from "@/lib/history";
import { useDelayed } from "@/lib/use-delayed";

const MY_AVATAR = "myAvatar";

type FilterUrl = {
  filters: Filters;
  /** applies a change from page 1 */
  apply: (patch: Partial<Filters>) => void;
  /** `noSemantic`: the first page went without meaning-based matches, so later pages must too */
  goPage: (page: number, noSemantic: boolean) => void;
};

type Refresh = {
  /** a navigation is under way; the results on screen belong to the previous URL */
  pending: boolean;
  /** it has lasted long enough to show (lib/use-delayed): the indicator is up, and the new results fade in */
  shown: boolean;
};

const Ctx = createContext<FilterUrl | null>(null);
const RefreshCtx = createContext<Refresh>({ pending: false, shown: false });

function remember(avatar: string) {
  try {
    localStorage.setItem(MY_AVATAR, avatar);
  } catch {}
}

function remembered(): string {
  try {
    return localStorage.getItem(MY_AVATAR) ?? "";
  } catch {
    return "";
  }
}

export function FilterUrlProvider({
  filters,
  children,
}: {
  filters: Filters;
  children: React.ReactNode;
}) {
  const router = useRouter();
  const pathname = usePathname();
  const [pending, startTransition] = useTransition();
  const shown = useDelayed(pending);
  const refresh = useMemo(() => ({ pending, shown }), [pending, shown]);

  // a bare visit gets the avatar this browser picked last; a shared link shows exactly what it encodes
  useEffect(() => {
    const avatar = remembered();
    if (location.search.length <= 1 && avatar)
      router.replace(
        `${pathname}?${filtersToQuery({ ...DEFAULT_FILTERS, avatar })}`,
        { scroll: false },
      );
  }, [pathname, router]);

  const value = useMemo<FilterUrl>(() => {
    const go = (query: string, push: boolean) => {
      const url = query ? `${pathname}?${query}` : pathname;
      startTransition(() =>
        push
          ? router.push(url, { scroll: false })
          : router.replace(url, { scroll: false }),
      );
      window.scrollTo({ top: 0 });
    };
    return {
      filters,
      apply: (patch) => {
        // a search, shop or avatar picked in the recent view leaves it, as on master
        const view = filters.view === "recent" &&
          !patch.view && { view: "items" as const };
        go(filtersToQuery({ ...filters, ...view, ...patch }), false);
        if (patch.q) saveHistory(addQuery(loadHistory(), patch.q));
        if (patch.avatar !== undefined) remember(patch.avatar);
      },
      // push, not replace: Back returns to the previous page
      goPage: (page, noSemantic) =>
        go(
          [
            filtersToQuery(filters),
            page > 1 ? `page=${page}` : "",
            page > 1 && noSemantic ? "sem=0" : "",
          ]
            .filter(Boolean)
            .join("&"),
          true,
        ),
    };
  }, [filters, pathname, router]);

  return (
    <Ctx value={value}>
      <RefreshCtx value={refresh}>{children}</RefreshCtx>
    </Ctx>
  );
}

export const useRefresh = (): Refresh => useContext(RefreshCtx);

export function useFilterUrl(): FilterUrl {
  const v = useContext(Ctx);
  if (!v) throw new Error("useFilterUrl outside FilterUrlProvider");
  return v;
}
