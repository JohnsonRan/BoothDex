"use client";

import { Search, SlidersHorizontal, X } from "lucide-react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
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
import { ApiError, api } from "@/lib/api";
import {
  type Avatar,
  avatarName,
  type Category,
  FEATURES,
  type Filters,
  filtersToQuery,
  type Item,
  parseFilters,
  type SearchResult,
  searchQuery,
} from "@/lib/catalog";
import { AvatarStrip } from "./avatar-strip";
import { CommandPalette } from "./command-palette";
import { activeFilterCount, FilterPanel } from "./filter-panel";
import { ItemCard } from "./item-card";
import { ItemDetail } from "./item-detail";

/** stable keys for loading placeholders */
const SKELETONS = Array.from({ length: 12 }, (_, i) => `skeleton-${i}`);

const SORTS = [
  ["trending", "热门"],
  ["newest", "最新"],
  ["likes", "点赞最多"],
  ["price:asc", "价格从低到高"],
  ["price:desc", "价格从高到低"],
] as const;

export function Browse() {
  const router = useRouter();
  const pathname = usePathname();
  const params = useSearchParams();
  const query = params.toString();
  const filters = useMemo(() => parseFilters(query), [query]);

  const [avatars, setAvatars] = useState<Avatar[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [items, setItems] = useState<Item[]>([]);
  const [meta, setMeta] = useState<Pick<
    SearchResult,
    "count" | "smartCounts" | "featureCounts"
  > | null>(null);
  const [loading, setLoading] = useState(true);
  const [favs, setFavs] = useState<Set<number> | null>(null);
  const [open, setOpen] = useState<Item | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const paging = useRef({ page: 0, total: 1, busy: false, key: "" });
  const sentinel = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api<{ items: Avatar[] }>("/api/avatars?limit=60").then((r) =>
      setAvatars(r.items),
    );
    api<{ categories: Category[] }>("/api/categories").then((r) =>
      setCategories(r.categories),
    );
    api<{ ids: number[] }>("/api/account/favourites")
      .then((r) => setFavs(new Set(r.ids)))
      .catch(() => setFavs(null));
  }, []);

  const update = useCallback(
    (patch: Partial<Filters>) => {
      const next = filtersToQuery({ ...filters, ...patch });
      router.replace(next ? `${pathname}?${next}` : pathname, {
        scroll: false,
      });
      window.scrollTo({ top: 0 });
    },
    [filters, pathname, router],
  );

  // one page of results; a filter change starts a new "key" so late responses for old filters are dropped
  const loadMore = useCallback(async () => {
    const p = paging.current;
    if (p.busy || p.page >= p.total) return;
    p.busy = true;
    const key = p.key;
    try {
      const r = await api<SearchResult>(
        `/api/search?${searchQuery(filters, p.page + 1)}`,
      );
      if (key !== paging.current.key) return;
      p.page += 1;
      p.total = r.totalPages;
      setItems((prev) => (p.page === 1 ? r.items : [...prev, ...r.items]));
      if (p.page === 1) setMeta(r);
    } catch (e) {
      if (key === paging.current.key)
        toast.error(e instanceof Error ? e.message : "加载失败");
    } finally {
      if (key === paging.current.key) {
        p.busy = false;
        setLoading(false);
        // IntersectionObserver only fires on changes: on tall screens the sentinel can stay in view after a
        // page renders, so keep filling until it is pushed below the fold
        requestAnimationFrame(() => {
          const el = sentinel.current;
          if (el && el.getBoundingClientRect().top < window.innerHeight + 800)
            loadMoreRef.current();
        });
      }
    }
  }, [filters]);
  const loadMoreRef = useRef(loadMore);
  loadMoreRef.current = loadMore;

  useEffect(() => {
    paging.current = { page: 0, total: 1, busy: false, key: query };
    setLoading(true);
    setItems([]);
    loadMore();
  }, [query, loadMore]);

  useEffect(() => {
    const el = sentinel.current;
    if (!el) return;
    const io = new IntersectionObserver(
      (e) => e[0].isIntersecting && loadMore(),
      { rootMargin: "800px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [loadMore]);

  const toggleFav = useCallback(
    async (it: Item) => {
      if (!favs) return toast("登录后才能收藏");
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
            ? "登录后才能收藏"
            : "收藏失败",
        );
      }
    },
    [favs],
  );

  const openById = async (id: number) => {
    const r = await api<{ items: Item[] }>(`/api/items?ids=${id}`);
    if (r.items[0]) setOpen(r.items[0]);
  };

  const myAvatar = avatars.find((a) => a.slug === filters.avatar) ?? null;
  const catName = (slug: string) =>
    categories.flatMap((c) => [c, ...c.children]).find((c) => c.slug === slug)
      ?.ja ?? slug;
  const chips: [string, Partial<Filters>][] = [
    ...(filters.q
      ? [[`“${filters.q}”`, { q: "" }] as [string, Partial<Filters>]]
      : []),
    ...(filters.author
      ? [[filters.author, { author: "" }] as [string, Partial<Filters>]]
      : []),
    ...(filters.sc
      ? [[catName(filters.sc), { sc: "" }] as [string, Partial<Filters>]]
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
          className="flex shrink-0 items-center gap-2 font-extrabold tracking-tight"
        >
          <span className="size-[18px] rounded-[5px] bg-brand" />
          <span className="hidden sm:inline">Catalogue</span>
        </a>
        <button
          type="button"
          onClick={() => setPaletteOpen(true)}
          className="flex h-9 min-w-0 flex-1 items-center gap-2 rounded-lg border bg-card px-3 text-sm text-muted-foreground sm:max-w-xl"
        >
          <Search className="size-4 shrink-0" />
          <span className="truncate">搜索商品、头像、作者、标签</span>
          <kbd className="ml-auto hidden rounded border px-1.5 font-sans text-[11px] sm:inline">
            Ctrl K
          </kbd>
        </button>
        <button
          type="button"
          onClick={() => setDrawerOpen(true)}
          className="relative grid size-9 shrink-0 place-items-center rounded-lg border bg-card lg:hidden"
          aria-label={`筛选${nActive ? `（已选 ${nActive} 项）` : ""}`}
        >
          <SlidersHorizontal className="size-4" />
          {nActive > 0 && (
            <span className="absolute -top-1.5 -right-1.5 grid size-5 place-items-center rounded-full bg-primary text-[10px] font-bold text-primary-foreground">
              {nActive}
            </span>
          )}
        </button>
      </header>

      <section className="border-b bg-[radial-gradient(900px_260px_at_75%_-40%,color-mix(in_oklch,var(--brand-2)_18%,transparent),transparent_70%)] px-3 pt-4 sm:px-5">
        <div className="flex items-baseline gap-3">
          <h1 className="min-w-0 truncate text-lg font-bold tracking-tight sm:text-[22px]">
            {filters.q ? (
              <>
                搜索 <span className="text-brand">{filters.q}</span>
              </>
            ) : filters.author ? (
              <>
                <span className="text-brand">{filters.author}</span> 的商品
              </>
            ) : myAvatar ? (
              <>
                适配 <span className="text-brand">{avatarName(myAvatar)}</span>{" "}
                的商品
              </>
            ) : (
              "全部商品"
            )}
          </h1>
          <span className="shrink-0 text-sm text-muted-foreground tabular-nums">
            {meta ? `${meta.count.toLocaleString()} 件` : ""}
          </span>
          {filters.avatar && (
            <button
              type="button"
              onClick={() => update({ avatar: "" })}
              className="ml-auto flex shrink-0 items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
            >
              <X className="size-3.5" />
              <span className="hidden sm:inline">不限头像</span>
            </button>
          )}
        </div>
        <AvatarStrip
          avatars={avatars}
          value={filters.avatar}
          onChange={(slug) => update({ avatar: slug })}
        />
      </section>

      <div className="lg:grid lg:grid-cols-[232px_1fr]">
        <aside className="sticky top-14 hidden h-[calc(100dvh-3.5rem)] overflow-y-auto border-r px-4 py-5 lg:block">
          {panel}
        </aside>
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
                aria-label="排序"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent align="end">
                {SORTS.map(([v, label]) => (
                  <SelectItem key={v} value={v}>
                    {label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3 sm:gap-4 lg:grid-cols-[repeat(auto-fill,minmax(200px,1fr))]">
            {items.map((it) => (
              <ItemCard
                key={it.id}
                item={it}
                favourite={!!favs?.has(it.id)}
                onOpen={setOpen}
                onFavourite={toggleFav}
              />
            ))}
            {loading &&
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
          {!loading && items.length === 0 && (
            <p className="py-20 text-center text-muted-foreground">
              没有找到商品
            </p>
          )}
          <div ref={sentinel} className="h-px" />
        </main>
      </div>

      <Drawer open={drawerOpen} onOpenChange={setDrawerOpen}>
        <DrawerContent>
          <DrawerHeader>
            <DrawerTitle>筛选</DrawerTitle>
          </DrawerHeader>
          <div className="overflow-y-auto px-4">{panel}</div>
          <DrawerFooter className="flex-row">
            <button
              type="button"
              onClick={() => update({ sc: "", features: [], adult: "safe" })}
              className="flex-1 rounded-lg border py-2.5 text-sm"
            >
              清除
            </button>
            <button
              type="button"
              onClick={() => setDrawerOpen(false)}
              className="flex-[2] rounded-lg bg-brand py-2.5 text-sm font-semibold text-white"
            >
              查看 {meta ? meta.count.toLocaleString() : ""} 件商品
            </button>
          </DrawerFooter>
        </DrawerContent>
      </Drawer>

      <ItemDetail
        item={open}
        myAvatar={myAvatar}
        favourite={!!open && !!favs?.has(open.id)}
        adult={filters.adult}
        onClose={() => setOpen(null)}
        onFavourite={toggleFav}
        onPickAvatar={(slug) => {
          setOpen(null);
          update({ avatar: slug });
        }}
        onSearch={(patch) => {
          setOpen(null);
          update({ q: "", author: "", sc: "", avatar: "", ...patch });
        }}
        onOpenItem={setOpen}
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
