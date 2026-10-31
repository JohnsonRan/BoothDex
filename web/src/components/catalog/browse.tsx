"use client";

import { Loader2, Search, SlidersHorizontal, Trash2, X } from "lucide-react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Drawer,
  DrawerContent,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { ApiError, api, apiReady } from "@/lib/api";
import {
  type Avatar,
  avatarName,
  type Category,
  DEFAULT_FILTERS,
  FEATURES,
  type Filters,
  filtersToQuery,
  type Item,
  localName,
  parseFilters,
  type SearchResult,
  searchQuery,
} from "@/lib/catalog";
import { addQuery, loadHistory, saveHistory } from "@/lib/history";
import type { MessageKey } from "@/lib/messages";
import { useSettings } from "@/lib/settings";
import { cn } from "@/lib/utils";
import {
  addViewed,
  loadViewed,
  recordLocalView,
  saveViewed,
} from "@/lib/viewed";
import { AccountMenu, type Me } from "./account-menu";
import { AvatarStrip } from "./avatar-strip";
import { CommandPalette } from "./command-palette";
import { activeFilterCount, FilterPanel } from "./filter-panel";
import { ItemCard } from "./item-card";
import { ItemDetail } from "./item-detail";
import { Logo } from "./logo";
import { Pagination } from "./pagination";
import { SearchTrace, type TraceMeta } from "./search-trace";
import { SettingsMenu } from "./settings-menu";

const PAGE_SIZE = 100;

/** stable keys for loading placeholders */
const SKELETONS = Array.from({ length: 12 }, (_, i) => `skeleton-${i}`);

const SORTS = [
  ["trending", "sort.trending"],
  ["newest", "sort.newest"],
  ["likes", "sort.likes"],
  ["price:asc", "sort.priceAsc"],
  ["price:desc", "sort.priceDesc"],
] as const;

export function Browse() {
  const { t, ui } = useSettings();
  // read inside stable callbacks so switching language does not refetch results
  const tRef = useRef(t);
  tRef.current = t;
  const router = useRouter();
  const pathname = usePathname();
  const params = useSearchParams();
  // the open item lives in the URL too (shareable, Back closes it) but is not part of the search
  const itemParam = Number(params.get("item")) || 0;
  const query = useMemo(() => {
    const p = new URLSearchParams(params);
    p.delete("item");
    return p.toString();
  }, [params]);
  const filters = useMemo(() => parseFilters(query), [query]);
  // the page lives in the URL next to the filters; changing a filter drops it, so a new search starts at 1
  const page = Math.max(1, Number.parseInt(params.get("page") ?? "", 10) || 1);
  const filterKey = filtersToQuery(filters);

  const [avatars, setAvatars] = useState<Avatar[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [items, setItems] = useState<Item[]>([]);
  const [meta, setMeta] = useState<
    | (Pick<SearchResult, "count" | "smartCounts" | "featureCounts"> &
        TraceMeta)
    | null
  >(null);
  const [loading, setLoading] = useState(true);
  const [favs, setFavs] = useState<Set<number> | null>(null);
  // opened items, newest first (null until the account or this browser's list is read)
  const [viewed, setViewed] = useState<number[] | null>(null);
  const [viewedGen, setViewedGen] = useState(0);
  const viewedRef = useRef(viewed);
  viewedRef.current = viewed;
  const viewedSet = useMemo(() => new Set(viewed), [viewed]);
  const recent = filters.view === "recent";
  const recentKey = recent ? viewedGen : 0;
  // recent cards only while they belong to the current owner's list: a sign-out or switch hides them at once
  const shownItems = recent
    ? items.filter((it) => viewedSet.has(it.id))
    : items;
  const [open, setOpen] = useState<Item | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [drawerOpen, setDrawerOpen] = useState(false);
  // the page the API answered (it clamps one past the end) and how many there are
  const [shown, setShown] = useState({ page: 1, total: 1 });
  // the first page fetched for a search went without meaning-based matches: its other pages must too, or the
  // result set would change between pages
  const semantic = useRef({ key: "", off: false });
  // latest request; answers to older ones are dropped
  const request = useRef(0);
  // "starting": the API is still loading after a restart; "error": it is up but the request failed
  const [apiState, setApiState] = useState<"ready" | "starting" | "error">(
    "ready",
  );
  // bumped to reload everything once the API is back
  const [epoch, setEpoch] = useState(0);

  const [me, setMe] = useState<Me | null>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: epoch only triggers a reload
  useEffect(() => {
    api<{ items: Avatar[] }>("/api/avatars?limit=60")
      .then((r) => setAvatars(r.items))
      .catch(() => {});
    api<{ categories: Category[] }>("/api/categories")
      .then((r) => setCategories(r.categories))
      .catch(() => {});
  }, [epoch]);

  // latest account load; answers to older ones are dropped
  const accountLoad = useRef(0);

  // session: who am I, my favourites, what I viewed; rerun after login/logout
  const loadAccount = useCallback(async () => {
    const gen = ++accountLoad.current;
    setViewed(null);
    const user = await api<Me>("/api/account").catch(() => null);
    // a newer sign-in/out supersedes this load; don't publish another account's data
    if (gen !== accountLoad.current) return user;
    setMe(user);
    if (user) {
      // what this browser viewed while signed out moves into the account
      const local = loadViewed();
      if (local.length)
        await api("/api/account/views", {
          method: "POST",
          json: { views: local.map(([id, at]) => ({ id, at })) },
        })
          .then(() => saveViewed([]))
          .catch(() => {});
      if (gen !== accountLoad.current) return user;
      const res = await api<{ ids: number[] }>("/api/account/views").catch(
        () => ({
          ids: [],
        }),
      );
      if (gen !== accountLoad.current) return user;
      setViewed((cur) => [
        ...(cur ?? []),
        ...res.ids.filter((id) => !cur?.includes(id)),
      ]);
    } else {
      setViewed(loadViewed().map(([id]) => id));
    }
    setViewedGen((g) => g + 1);
    const favRes = user
      ? await api<{ ids: number[] }>("/api/account/favourites").catch(() => ({
          ids: [],
        }))
      : null;
    if (gen !== accountLoad.current) return user;
    setFavs(favRes ? new Set(favRes.ids) : null);
    return user;
  }, []);

  const update = useCallback(
    (patch: Partial<Filters>) => {
      // search, author or avatar filters picked from palette or detail leave the recent view
      const viewPatch =
        filters.view === "recent" && !patch.view
          ? { view: "items" as const }
          : {};
      const next = filtersToQuery({ ...filters, ...viewPatch, ...patch });
      router.replace(next ? `${pathname}?${next}` : pathname, {
        scroll: false,
      });
      window.scrollTo({ top: 0 });
      if (patch.q) saveHistory(addQuery(loadHistory(), patch.q));
      // remember "my avatar" (account preference when signed in, otherwise this browser)
      if (patch.avatar !== undefined) {
        localStorage.setItem("myAvatar", patch.avatar);
        if (me)
          api("/api/account/preferences", {
            method: "PUT",
            json: { avatar: patch.avatar },
          }).catch(() => {});
      }
    },
    [filters, pathname, router, me],
  );

  // first visit without an avatar in the URL: restore the remembered one
  const restored = useRef(false);
  useEffect(() => {
    loadAccount().then((user) => {
      if (restored.current) return;
      restored.current = true;
      const saved =
        user?.preferences?.avatar ?? localStorage.getItem("myAvatar") ?? "";
      // only a bare visit gets the remembered avatar; a shared link shows exactly what it encodes
      if (saved && location.search.length <= 1)
        router.replace(
          `${pathname}?${filtersToQuery({ ...DEFAULT_FILTERS, avatar: saved })}`,
          { scroll: false },
        );
    });
  }, [loadAccount, pathname, router]);

  // one page of results
  // biome-ignore lint/correctness/useExhaustiveDependencies: epoch only triggers a reload
  useEffect(() => {
    const id = ++request.current;
    const sem = semantic.current;
    const noSemantic = sem.key === filterKey && sem.off;
    const started = performance.now();
    // the previous results stay (dimmed) until the new ones arrive
    setLoading(true);
    if (filters.view === "recent" && viewedRef.current === null) return;
    const ids = (viewedRef.current ?? []).slice(0, PAGE_SIZE);
    const load: Promise<SearchResult> =
      filters.view !== "recent"
        ? api<SearchResult>(
            `/api/search?${searchQuery(filters, page, PAGE_SIZE)}${noSemantic ? "&sem=0" : ""}`,
          )
        : // newest first, one page; filters and sort don't apply
          (ids.length
            ? api<{ items: Item[] }>(`/api/items?ids=${ids.join(",")}`)
            : Promise.resolve({ items: [] as Item[] })
          ).then((r) => ({
            items: r.items,
            count: r.items.length,
            page: 1,
            totalPages: 1,
            smartCounts: {},
            featureCounts: {},
          }));
    load
      .then((r) => {
        if (id !== request.current) return;
        setItems(r.items);
        setShown({ page: r.page, total: r.totalPages });
        setMeta({ ...r, clientMs: performance.now() - started });
        setApiState("ready");
        if (sem.key !== filterKey)
          semantic.current = { key: filterKey, off: r.semantic !== "used" };
      })
      // one status line instead of a toast per attempt; no automatic retry from here
      .catch(async () => {
        if (id !== request.current) return;
        setItems([]);
        const ready = await apiReady();
        // a newer request may have succeeded while readiness was checked
        if (id === request.current) setApiState(ready ? "error" : "starting");
      })
      .finally(() => {
        if (id === request.current) setLoading(false);
      });
  }, [filters, filterKey, page, epoch, recentKey]);

  const goPage = useCallback(
    (n: number) => {
      const next = [filterKey, n > 1 ? `page=${n}` : ""]
        .filter(Boolean)
        .join("&");
      // push, not replace: the back button returns to the previous page
      router.push(next ? `${pathname}?${next}` : pathname, { scroll: false });
      window.scrollTo({ top: 0 });
    },
    [filterKey, pathname, router],
  );

  // wait for the API to finish starting, then load everything again
  useEffect(() => {
    if (apiState !== "starting") return;
    const timer = setInterval(async () => {
      if (await apiReady()) {
        setApiState("ready");
        setEpoch((n) => n + 1);
        loadAccount();
      }
    }, 3000);
    return () => clearInterval(timer);
  }, [apiState, loadAccount]);

  const toggleFav = useCallback(
    async (it: Item) => {
      if (!favs) return toast(tRef.current("fav.needSignIn"));
      const on = favs.has(it.id);
      setFavs((s) => {
        const n = new Set(s);
        on ? n.delete(it.id) : n.add(it.id);
        return n;
      });
      try {
        await api(`/api/account/favourites/${it.id}`, {
          method: on ? "DELETE" : "PUT",
        });
      } catch (e) {
        setFavs((s) => {
          const n = new Set(s);
          on ? n.add(it.id) : n.delete(it.id);
          return n;
        });
        toast.error(
          e instanceof ApiError && e.status === 401
            ? tRef.current("fav.needSignIn")
            : tRef.current("fav.failed"),
        );
      }
    },
    [favs],
  );

  // the item=<id> URL parameter: push on open so Back closes it, replace when moving between items
  const itemUrl = useCallback(
    (id: number, push: boolean) => {
      const p = new URLSearchParams(query);
      if (id) p.set("item", String(id));
      const next = p.toString();
      const url = next ? `${pathname}?${next}` : pathname;
      if (push) history.pushState(null, "", url);
      else history.replaceState(null, "", url);
    },
    [query, pathname],
  );
  const openedHere = useRef(false);
  // stable so the memoised cards do not all re-render
  const openItem = useCallback(
    (it: Item) => {
      setOpen(it);
      if (it.id === itemParam) return;
      if (!itemParam) openedHere.current = true;
      itemUrl(it.id, !itemParam);
    },
    [itemParam, itemUrl],
  );
  const closeItem = () => {
    setOpen(null);
    // step back over the entry opening pushed; a shared link has nothing to go back to
    if (openedHere.current) history.back();
    else itemUrl(0, false);
    openedHere.current = false;
  };
  // follow the URL: a shared link or Forward opens the item, Back closes it
  const openRef = useRef(open);
  openRef.current = open;
  // waits while the API starts and retries once it is back, whichever request (poll or search) notices first
  const starting = apiState === "starting";
  // biome-ignore lint/correctness/useExhaustiveDependencies: epoch and starting only trigger a retry
  useEffect(() => {
    if (!itemParam) {
      openedHere.current = false;
      setOpen(null);
      return;
    }
    if (openRef.current?.id === itemParam || starting) return;
    let live = true;
    api<{ items: Item[] }>(`/api/items?ids=${itemParam}`)
      .then((r) => {
        if (!live) return;
        if (r.items[0]) setOpen(r.items[0]);
        else {
          toast.error(tRef.current("list.loadFailed"));
          itemUrl(0, false);
        }
      })
      .catch(async () => {
        if (!live) return;
        const ready = await apiReady();
        if (!live) return;
        if (!ready) {
          setApiState("starting");
          return;
        }
        toast.error(tRef.current("list.loadFailed"));
        itemUrl(0, false);
      });
    return () => {
      live = false;
    };
  }, [itemParam, epoch, itemUrl, starting]);

  // remember what was opened: in the account when signed in, otherwise in this browser
  const openId = open?.id;
  useEffect(() => {
    if (!openId) return;
    setViewed((v) => addViewed(v ?? [], openId));
    if (me)
      api("/api/account/views", {
        method: "POST",
        json: { views: [{ id: openId }] },
      }).catch(() => {});
    else recordLocalView(openId);
  }, [openId, me]);

  const clearViewed = async () => {
    try {
      if (me) await api("/api/account/views", { method: "DELETE" });
      else saveViewed([]);
      setViewed([]);
    } catch {
      toast.error(t("recent.clearFailed"));
    }
  };

  const openById = (id: number) => {
    if (id === itemParam && open?.id === id) return;
    if (!itemParam) openedHere.current = true;
    itemUrl(id, !itemParam);
  };

  const myAvatar = avatars.find((a) => a.slug === filters.avatar) ?? null;
  // title with the variable part (query, shop, avatar) highlighted, in any UI language
  const highlight = (key: MessageKey, name: string, value: string) => {
    const [before, after] = t(key, { [name]: "\u0000" }).split("\u0000");
    return (
      <>
        {before}
        <span className="text-brand">{value}</span>
        {after}
      </>
    );
  };
  const catName = (slug: string) =>
    categories.flatMap((c) => [c, ...c.children]).find((c) => c.slug === slug);
  const catLabel = (slug: string) => {
    const c = catName(slug);
    return c ? localName(c, ui) : slug;
  };
  // plain-text title, used as the default name of a saved search
  const heading = [
    filters.q
      ? t("title.search", { q: filters.q })
      : filters.author
        ? t("title.author", { name: filters.author })
        : myAvatar
          ? t("title.avatar", { name: avatarName(myAvatar, ui) })
          : filters.view === "avatars"
            ? t("title.avatars")
            : t("title.all"),
    filters.sc && catLabel(filters.sc),
  ]
    .filter(Boolean)
    .join(" · ");
  const chips: [string, Partial<Filters>][] = [
    ...(filters.q
      ? [[`“${filters.q}”`, { q: "" }] as [string, Partial<Filters>]]
      : []),
    ...(filters.author
      ? [[filters.author, { author: "" }] as [string, Partial<Filters>]]
      : []),
    ...(filters.sc
      ? [[catLabel(filters.sc), { sc: "" }] as [string, Partial<Filters>]]
      : []),
    ...filters.features.map(
      (f) =>
        [
          FEATURES[f],
          { features: filters.features.filter((x) => x !== f) },
        ] as [string, Partial<Filters>],
    ),
  ];
  const nActive = activeFilterCount(filters);
  const sortValue =
    filters.sort === "price" ? `price:${filters.dir}` : filters.sort;
  const panel = (
    <FilterPanel
      filters={filters}
      categories={categories}
      smartCounts={meta?.smartCounts ?? {}}
      featureCounts={meta?.featureCounts ?? {}}
      onChange={update}
    />
  );

  return (
    <div className="min-h-dvh">
      <header className="sticky top-0 z-30 flex h-14 items-center gap-3 border-b bg-background/80 px-3 backdrop-blur-md sm:px-5">
        <a
          href="/"
          className="flex shrink-0 items-center gap-2 font-heading text-[17px] font-semibold tracking-wide"
        >
          <Logo />
          <span className="hidden sm:inline">BoothDex</span>
        </a>
        <button
          type="button"
          onClick={() => setPaletteOpen(true)}
          className="flex h-9 min-w-0 flex-1 items-center gap-2 rounded-lg border bg-card px-3 text-sm text-muted-foreground sm:max-w-xl"
        >
          <Search className="size-4 shrink-0" />
          <span className="truncate">{t("nav.search")}</span>
          <kbd className="ml-auto hidden rounded border px-1.5 font-sans text-[11px] sm:inline">
            Ctrl K
          </kbd>
        </button>
        <button
          type="button"
          onClick={() => setDrawerOpen(true)}
          className={cn(
            "relative grid size-9 shrink-0 place-items-center rounded-lg border bg-card lg:hidden",
            recent && "hidden",
          )}
          aria-label={
            nActive ? t("nav.filtersActive", { n: nActive }) : t("nav.filters")
          }
        >
          <SlidersHorizontal className="size-4" />
          {nActive > 0 && (
            <span className="absolute -top-1.5 -right-1.5 grid size-5 place-items-center rounded-full bg-primary text-[10px] font-bold text-primary-foreground">
              {nActive}
            </span>
          )}
        </button>
        <div className="ml-auto flex items-center gap-2">
          <SettingsMenu />
          <AccountMenu
            me={me}
            onAuthChange={loadAccount}
            onShowFavourites={() =>
              update({
                view: "favourites",
                avatar: "",
                q: "",
                author: "",
                sc: "",
                features: [],
              })
            }
            // keeps the avatar, so going back to all shows it again
            onShowRecent={() =>
              update({
                view: "recent",
                q: "",
                author: "",
                sc: "",
                features: [],
              })
            }
            currentQuery={filtersToQuery({ ...filters, view: "items" })}
            currentLabel={heading}
          />
        </div>
      </header>

      <section className="border-b px-3 pt-4 sm:px-5">
        <div className="flex items-baseline gap-3">
          <h1 className="min-w-0 truncate font-heading text-xl font-semibold tracking-wide sm:text-[26px]">
            {filters.view === "favourites" ? (
              <span className="text-brand">{t("title.favourites")}</span>
            ) : recent ? (
              <span className="text-brand">{t("title.recent")}</span>
            ) : filters.q ? (
              highlight("title.search", "q", filters.q)
            ) : filters.author ? (
              highlight("title.author", "name", filters.author)
            ) : myAvatar ? (
              highlight("title.avatar", "name", avatarName(myAvatar, ui))
            ) : filters.view === "avatars" ? (
              t("title.avatars")
            ) : (
              t("title.all")
            )}
          </h1>
          <span className="shrink-0 text-sm text-muted-foreground tabular-nums">
            {meta
              ? t("title.count", {
                  n: (recent ? shownItems.length : meta.count).toLocaleString(),
                })
              : ""}
          </span>
          {meta && filters.q && <SearchTrace meta={meta} />}
          {recent && !!viewed?.length && (
            <button
              type="button"
              onClick={clearViewed}
              aria-label={t("recent.clear")}
              className="ml-auto flex shrink-0 items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
            >
              <Trash2 className="size-3.5" />
              <span className="hidden sm:inline">{t("recent.clear")}</span>
            </button>
          )}
          {(filters.view === "favourites" || recent) && (
            <button
              type="button"
              onClick={() => update({ view: "items" })}
              className={cn(
                "flex shrink-0 items-center gap-1 text-sm text-muted-foreground hover:text-foreground",
                !(recent && viewed?.length) && "ml-auto",
              )}
            >
              <X className="size-3.5" />
              <span className="hidden sm:inline">{t("title.backToAll")}</span>
            </button>
          )}
          {filters.avatar && filters.view !== "favourites" && !recent && (
            <button
              type="button"
              onClick={() => update({ avatar: "" })}
              className="ml-auto flex shrink-0 items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
            >
              <X className="size-3.5" />
              <span className="hidden sm:inline">{t("title.anyAvatar")}</span>
            </button>
          )}
        </div>
        {filters.view === "items" && (
          <AvatarStrip
            avatars={avatars}
            value={filters.avatar}
            onChange={(slug) => update({ avatar: slug })}
          />
        )}
        {filters.view !== "items" && <div className="h-3" />}
      </section>

      <div className={cn(!recent && "lg:grid lg:grid-cols-[232px_1fr]")}>
        {!recent && (
          <aside className="sticky top-14 hidden h-[calc(100dvh-3.5rem)] overflow-y-auto border-r px-4 py-5 lg:block">
            {panel}
          </aside>
        )}
        <main className="min-w-0 px-3 pt-4 pb-16 sm:px-5 lg:px-6">
          <div className="mb-4 flex flex-wrap items-center gap-2">
            {chips.map(([label, patch]) => (
              <button
                key={label}
                type="button"
                onClick={() => update(patch)}
                className="flex items-center gap-1 rounded-full border bg-card px-2.5 py-1 text-xs hover:bg-accent"
              >
                {label}
                <X className="size-3" />
              </button>
            ))}
            {!recent && (
              <Select
                value={sortValue}
                onValueChange={(v) => {
                  const [sort, dir] = v.split(":") as [
                    Filters["sort"],
                    Filters["dir"] | undefined,
                  ];
                  update({ sort, dir: dir ?? "desc" });
                }}
              >
                <SelectTrigger
                  size="sm"
                  className="ml-auto w-36"
                  aria-label={t("sort.label")}
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent align="end">
                  {SORTS.map(([v, label]) => (
                    <SelectItem key={v} value={v}>
                      {/* the default sort ranks a text search by relevance (see searchQuery) */}
                      {t(
                        (v === "trending" && filters.q
                          ? "sort.relevant"
                          : label) as MessageKey,
                      )}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </div>

          <div
            aria-busy={loading}
            inert={loading && shownItems.length > 0}
            className={cn(
              "grid grid-cols-2 gap-2.5 transition-opacity sm:grid-cols-3 sm:gap-4 lg:grid-cols-[repeat(auto-fill,minmax(200px,1fr))]",
              loading && shownItems.length > 0 && "opacity-50",
            )}
          >
            {shownItems.map((it) => (
              <ItemCard
                key={it.id}
                item={it}
                favourite={!!favs?.has(it.id)}
                viewed={!recent && viewedSet.has(it.id)}
                onOpen={openItem}
                onFavourite={toggleFav}
              />
            ))}
            {loading &&
              shownItems.length === 0 &&
              SKELETONS.map((k) => (
                <div
                  key={k}
                  className="overflow-hidden rounded-xl border bg-card"
                >
                  <Skeleton className="aspect-square rounded-none" />
                  <div className="space-y-2 p-3">
                    <Skeleton className="h-3.5 w-full" />
                    <Skeleton className="h-3.5 w-2/3" />
                  </div>
                </div>
              ))}
          </div>
          {apiState === "starting" && (
            <output className="flex items-center justify-center gap-2 py-20 text-center text-muted-foreground">
              <Loader2 className="size-4 animate-spin" aria-hidden />
              {t("list.starting")}
            </output>
          )}
          {apiState === "error" && (
            <div role="alert" className="space-y-3 py-20 text-center">
              <p className="text-muted-foreground">{t("list.loadFailed")}</p>
              <Button
                variant="outline"
                onClick={() => {
                  setApiState("ready");
                  setEpoch((n) => n + 1);
                }}
              >
                {t("list.retry")}
              </Button>
            </div>
          )}
          {meta?.semantic === "unavailable" && (
            <p className="pb-3 text-center text-muted-foreground text-xs">
              {t("list.semanticUnavailable")}
            </p>
          )}
          {apiState === "ready" && !loading && shownItems.length === 0 && (
            <p className="py-20 text-center text-muted-foreground">
              {t(recent ? "recent.empty" : "list.empty")}
            </p>
          )}
          <Pagination page={shown.page} total={shown.total} onPage={goPage} />
        </main>
      </div>

      <Drawer open={drawerOpen} onOpenChange={setDrawerOpen}>
        <DrawerContent>
          <DrawerHeader>
            <DrawerTitle>{t("nav.filters")}</DrawerTitle>
          </DrawerHeader>
          <div className="overflow-y-auto px-4">{panel}</div>
          <DrawerFooter className="flex-row">
            <button
              type="button"
              onClick={() =>
                update({
                  sc: "",
                  features: [],
                  adult: "safe",
                  tags: [],
                  extags: [],
                  free: false,
                  pmin: 0,
                  pmax: 0,
                  range: "",
                  gender: "",
                  asup: "supported",
                })
              }
              className="flex-1 rounded-lg border py-2.5 text-sm"
            >
              {t("filter.clear")}
            </button>
            <button
              type="button"
              onClick={() => setDrawerOpen(false)}
              className="flex-[2] rounded-lg bg-brand py-2.5 text-sm font-semibold text-white"
            >
              {t("filter.show", { n: meta ? meta.count.toLocaleString() : "" })}
            </button>
          </DrawerFooter>
        </DrawerContent>
      </Drawer>

      <ItemDetail
        item={open}
        myAvatar={myAvatar}
        favourite={!!open && !!favs?.has(open.id)}
        adult={filters.adult}
        onClose={closeItem}
        onFavourite={toggleFav}
        onPickAvatar={(slug) => {
          setOpen(null);
          openedHere.current = false;
          update({ avatar: slug });
        }}
        onSearch={(patch) => {
          setOpen(null);
          openedHere.current = false;
          update({ q: "", author: "", sc: "", avatar: "", ...patch });
        }}
        onOpenItem={openItem}
      />
      <CommandPalette
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
        adult={filters.adult}
        onQuery={(q) => update({ q })}
        onAvatar={(slug) => update({ avatar: slug })}
        onAuthor={(author) => update({ author, q: "", avatar: "" })}
        onItem={openById}
      />
    </div>
  );
}
