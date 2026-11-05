"use client";

import {
  ArrowLeft,
  Hash,
  History,
  Search,
  Store,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { api } from "@/lib/api";
import {
  type Avatar,
  avatarName,
  imageSrc,
  imgUrl,
  tagName,
} from "@/lib/catalog";
import { loadHistory, saveHistory } from "@/lib/history";
import { useSettings } from "@/lib/settings";

type PaletteResult = {
  items: {
    id: number;
    title: string;
    author_name: string;
    price_jpy: number | null;
    smart_price: number | null;
    image: string | null;
  }[];
  itemCount: number;
  avatars: (Avatar & { item_count: number })[];
  authors: { name: string; image_url: string | null; item_count: number }[];
  tags: {
    name: string;
    display: string;
    display_zh?: string | null;
    item_count: number;
  }[];
};

type Props = {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  adult: string;
  onQuery: (q: string) => void;
  onAvatar: (slug: string) => void;
  onAuthor: (name: string) => void;
  onItem: (id: number) => void;
};

/** Ctrl/⌘+K search across items, avatars, authors and tags. Full-screen on phones. */
export function CommandPalette({
  open,
  onOpenChange,
  adult,
  onQuery,
  onAvatar,
  onAuthor,
  onItem,
}: Props) {
  const { t, price, ui } = useSettings();
  const [q, setQ] = useState("");
  const [res, setRes] = useState<PaletteResult | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  // read on every open: searches made since the last open are recorded by the page
  useEffect(() => {
    if (open) setHistory(loadHistory());
  }, [open]);
  const forget = (h: string | null) => {
    const next = h === null ? [] : history.filter((x) => x !== h);
    setHistory(next);
    saveHistory(next);
  };

  useEffect(() => {
    if (!q.trim()) return setRes(null);
    const ctl = new AbortController();
    const t = setTimeout(() => {
      api<PaletteResult>(
        `/api/palette?q=${encodeURIComponent(q)}&adult=${adult}`,
        { signal: ctl.signal },
      )
        .then(setRes)
        .catch(() => {});
    }, 120);
    return () => {
      clearTimeout(t);
      ctl.abort();
    };
  }, [q, adult]);

  // Open = one history entry, so Back (the phone's back gesture) closes the palette instead of leaving the site.
  // Closing without a pick (arrow button, Esc, Ctrl+K) steps back; picks take the entry off before navigating.
  const entry = useRef(false);
  const close = useRef(onOpenChange);
  close.current = onOpenChange;
  useEffect(() => {
    if (open && !entry.current) {
      entry.current = true;
      window.history.pushState(null, "", location.href);
    } else if (!open && entry.current) {
      entry.current = false;
      window.history.back();
    }
    if (!open) return;
    const onBack = () => {
      // ignore popstate from our own run() back navigation
      if (!entry.current) return;
      entry.current = false;
      close.current(false);
    };
    addEventListener("popstate", onBack);
    return () => removeEventListener("popstate", onBack);
  }, [open]);

  const run = (fn: () => void) => {
    onOpenChange(false);
    setQ("");
    // pop the palette's entry first so item picks (which push) don't leave a duplicate entry behind
    if (entry.current) {
      entry.current = false;
      addEventListener("popstate", () => fn(), { once: true });
      window.history.back();
    } else {
      fn();
    }
  };
  const thumb = (url: string | null) =>
    url?.includes("booth.pximg.net")
      ? imgUrl({ booth_images: [url], media: [] }, 0, "72x72_a2_g5")
      : url
        ? imgUrl({ booth_images: null, media: [url] })
        : "";

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("palette.title")}
      description={t("palette.desc")}
      className="top-0 h-dvh max-w-none translate-y-0 rounded-none! sm:top-[12vh] sm:h-auto sm:max-w-xl sm:rounded-xl!"
    >
      <Command shouldFilter={false} className="h-full">
        <div className="flex items-center pr-1">
          {/* full screen on phones: no backdrop to tap and no Esc key, so offer a way out */}
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            aria-label={t("palette.close")}
            // 44px tap target; the negative margin keeps the header row as tall as the input
            className="-my-1 grid size-11 shrink-0 place-items-center pt-1 text-muted-foreground hover:text-foreground sm:hidden"
          >
            <ArrowLeft className="size-5" />
          </button>
          <div className="min-w-0 flex-1 max-sm:-ml-1">
            <CommandInput
              placeholder={t("palette.placeholder")}
              value={q}
              onValueChange={setQ}
              // 16px on phones: iOS zooms the page into smaller inputs on focus
              className="max-sm:text-base"
            />
          </div>
        </div>
        <CommandList className="max-h-none flex-1 sm:max-h-[60vh]">
          <CommandEmpty>
            {q.trim() ? t("palette.empty") : t("palette.hint")}
          </CommandEmpty>
          {!q.trim() && history.length > 0 && (
            <CommandGroup heading={t("history.title")}>
              {history.map((h) => (
                <CommandItem
                  key={h}
                  value={`history:${h}`}
                  onSelect={() => run(() => onQuery(h))}
                >
                  <History />
                  <span className="flex-1 truncate">{h}</span>
                  <button
                    type="button"
                    aria-label={t("history.remove", { q: h })}
                    className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
                    onPointerDown={(e) => e.preventDefault()}
                    onClick={(e) => {
                      e.stopPropagation();
                      forget(h);
                    }}
                  >
                    <X className="size-3.5" />
                  </button>
                </CommandItem>
              ))}
              <CommandItem value="history:clear" onSelect={() => forget(null)}>
                <Trash2 />
                <span className="text-muted-foreground">
                  {t("history.clear")}
                </span>
              </CommandItem>
            </CommandGroup>
          )}
          {res && q.trim() && (
            <>
              <CommandGroup>
                <CommandItem
                  value={`q:${q}`}
                  onSelect={() => run(() => onQuery(q.trim()))}
                >
                  <Search />
                  <span className="flex-1">
                    {t("palette.searchFor", { q: q.trim() })}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {t("palette.results", {
                      n: res.itemCount.toLocaleString(),
                    })}
                  </span>
                </CommandItem>
              </CommandGroup>
              {res.avatars.length > 0 && (
                <CommandGroup heading={t("palette.avatars")}>
                  {res.avatars.map((a) => (
                    <CommandItem
                      key={a.slug}
                      value={`a:${a.slug}`}
                      onSelect={() => run(() => onAvatar(a.slug))}
                    >
                      {a.image_url && (
                        <img
                          src={imageSrc(a.image_url)}
                          alt=""
                          className="size-8 rounded-md object-cover"
                        />
                      )}
                      <span className="flex-1 truncate">
                        {avatarName(a, ui)}{" "}
                        {/* the other common name, so either spelling is recognisable */}
                        <span className="text-muted-foreground">
                          {ui === "en" ? a.ja : a.en}
                        </span>
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {t("palette.fitting", {
                          n: a.item_count.toLocaleString(),
                        })}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              )}
              {res.items.length > 0 && (
                <CommandGroup heading={t("palette.items")}>
                  {res.items.map((it) => (
                    <CommandItem
                      key={it.id}
                      value={`i:${it.id}`}
                      onSelect={() => run(() => onItem(it.id))}
                    >
                      <img
                        src={thumb(it.image)}
                        alt=""
                        className="size-8 rounded-md bg-muted object-cover"
                      />
                      <span className="flex-1 truncate">{it.title}</span>
                      <span className="text-xs text-muted-foreground">
                        {price(it)}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              )}
              {res.authors.length > 0 && (
                <CommandGroup heading={t("palette.authors")}>
                  {res.authors.map((a) => (
                    <CommandItem
                      key={a.name}
                      value={`s:${a.name}`}
                      onSelect={() => run(() => onAuthor(a.name))}
                    >
                      {a.image_url ? (
                        <img
                          src={imageSrc(a.image_url)}
                          alt=""
                          className="size-8 rounded-full object-cover"
                        />
                      ) : (
                        <Store />
                      )}
                      <span className="flex-1 truncate">{a.name}</span>
                      <span className="text-xs text-muted-foreground">
                        {t("palette.itemsBy", { n: a.item_count })}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              )}
              {res.tags.length > 0 && (
                <CommandGroup heading={t("palette.tags")}>
                  {res.tags.map((tag) => (
                    <CommandItem
                      key={tag.name}
                      value={`t:${tag.name}`}
                      onSelect={() => run(() => onQuery(tag.display))}
                    >
                      <Hash />
                      <span className="flex-1 truncate">
                        {tagName(tag, ui)}
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {tag.item_count.toLocaleString()}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              )}
            </>
          )}
        </CommandList>
      </Command>
    </CommandDialog>
  );
}
