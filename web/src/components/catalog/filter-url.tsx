"use client";
// Filter state lives in the URL. The server parses it and renders the results; changing a filter navigates,
// and the new results arrive as a server render inside a transition, so the old ones stay until then. The
// controls show the change at once (an optimistic copy of the filters, replaced by the server's when it arrives).
// Whether a navigation is under way has its own context (useRefresh), so the controls do not render again when it
// starts or ends.

import { usePathname, useRouter } from "next/navigation";
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useOptimistic,
  useTransition,
} from "react";
import { fetchAccount } from "@/lib/api";
import { DEFAULT_FILTERS, type Filters, filtersToQuery } from "@/lib/catalog";
import { addQuery, loadHistory, saveHistory } from "@/lib/history";
import { saveAvatar } from "@/lib/preferences";
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
  const [current, setCurrent] = useOptimistic(filters);
  const shown = useDelayed(pending);
  const refresh = useMemo(() => ({ pending, shown }), [pending, shown]);

  // A bare visit gets the avatar this browser picked last; a shared link shows exactly what it encodes. The
  // account's pick (made on another device) follows, unless the visitor has moved on meanwhile, as on master.
  useEffect(() => {
    const query = (avatar: string) =>
      `?${filtersToQuery({ ...DEFAULT_FILTERS, avatar })}`;
    const restore = (avatar: string) =>
      router.replace(`${pathname}${query(avatar)}`, { scroll: false });
    const local = remembered();
    if (location.search.length <= 1 && local) restore(local);
    fetchAccount<{ preferences: Record<string, string> }>().then(
      ({ preferences }) => {
        const saved = preferences.avatar;
        const untouched =
          location.search.length <= 1 ||
          (!!local && location.search === query(local));
        if (saved && saved !== local && untouched) restore(saved);
      },
      () => {},
    );
  }, [pathname, router]);

  const value = useMemo<FilterUrl>(() => {
    const go = (query: string, push: boolean, next = current) => {
      const url = query ? `${pathname}?${query}` : pathname;
      startTransition(() => {
        setCurrent(next);
        if (push) router.push(url, { scroll: false });
        else router.replace(url, { scroll: false });
      });
      window.scrollTo({ top: 0 });
    };
    return {
      filters: current,
      apply: (patch) => {
        // a search, shop or avatar picked in the recent view leaves it, as on master
        const view = current.view === "recent" &&
          !patch.view && { view: "items" as const };
        const next = { ...current, ...view, ...patch };
        go(filtersToQuery(next), false, next);
        if (patch.q) saveHistory(addQuery(loadHistory(), patch.q));
        if (patch.avatar !== undefined) {
          remember(patch.avatar);
          saveAvatar(patch.avatar);
        }
      },
      // push, not replace: Back returns to the previous page
      goPage: (page, noSemantic) =>
        go(
          [
            filtersToQuery(current),
            page > 1 ? `page=${page}` : "",
            page > 1 && noSemantic ? "sem=0" : "",
          ]
            .filter(Boolean)
            .join("&"),
          true,
        ),
    };
  }, [current, setCurrent, pathname, router]);

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
