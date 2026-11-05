"use client";
// Filter state lives in the URL. The server parses it and renders the results; changing a filter navigates,
// and the new results arrive as a server render inside a transition, so the old ones stay until then.

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

const MY_AVATAR = "myAvatar";

type FilterUrl = {
  filters: Filters;
  /** a navigation is under way; the results on screen belong to the previous URL */
  pending: boolean;
  /** applies a change from page 1 */
  apply: (patch: Partial<Filters>) => void;
  /** `noSemantic`: the first page went without meaning-based matches, so later pages must too */
  goPage: (page: number, noSemantic: boolean) => void;
};

const Ctx = createContext<FilterUrl | null>(null);

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
      pending,
      apply: (patch) => {
        go(filtersToQuery({ ...filters, ...patch }), false);
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
  }, [filters, pending, pathname, router]);

  return <Ctx value={value}>{children}</Ctx>;
}

export function useFilterUrl(): FilterUrl {
  const v = useContext(Ctx);
  if (!v) throw new Error("useFilterUrl outside FilterUrlProvider");
  return v;
}
