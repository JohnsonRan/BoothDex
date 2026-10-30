"use client";

import { Hash, Search, Store } from "lucide-react";
import { useEffect, useState } from "react";
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
import { type Avatar, avatarName, imgUrl, priceText } from "@/lib/catalog";

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
  tags: { name: string; display: string; item_count: number }[];
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
  const [q, setQ] = useState("");
  const [res, setRes] = useState<PaletteResult | null>(null);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        onOpenChange(!open);
      }
    };
    addEventListener("keydown", key);
    return () => removeEventListener("keydown", key);
  }, [open, onOpenChange]);

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

  const run = (fn: () => void) => {
    onOpenChange(false);
    setQ("");
    fn();
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
      title="快速搜索"
      description="搜索商品、头像、作者和标签"
      className="top-0 h-dvh max-w-none translate-y-0 rounded-none! sm:top-[12vh] sm:h-auto sm:max-w-xl sm:rounded-xl!"
    >
      <Command shouldFilter={false} className="h-full">
        <CommandInput
          placeholder="搜索商品、头像、作者、标签…"
          value={q}
          onValueChange={setQ}
        />
        <CommandList className="max-h-none flex-1 sm:max-h-[60vh]">
          <CommandEmpty>
            {q.trim() ? "没有结果" : "输入关键词开始搜索"}
          </CommandEmpty>
          {res && q.trim() && (
            <>
              <CommandGroup>
                <CommandItem
                  value={`q:${q}`}
                  onSelect={() => run(() => onQuery(q.trim()))}
                >
                  <Search />
                  <span className="flex-1">搜索 “{q.trim()}”</span>
                  <span className="text-xs text-muted-foreground">
                    {res.itemCount.toLocaleString()} 个结果
                  </span>
                </CommandItem>
              </CommandGroup>
              {res.avatars.length > 0 && (
                <CommandGroup heading="头像">
                  {res.avatars.map((a) => (
                    <CommandItem
                      key={a.slug}
                      value={`a:${a.slug}`}
                      onSelect={() => run(() => onAvatar(a.slug))}
                    >
                      {a.image_url && (
                        <img
                          src={a.image_url}
                          alt=""
                          className="size-8 rounded-md object-cover"
                        />
                      )}
                      <span className="flex-1 truncate">
                        {avatarName(a)}{" "}
                        <span className="text-muted-foreground">{a.en}</span>
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {a.item_count.toLocaleString()} 个适配商品
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              )}
              {res.items.length > 0 && (
                <CommandGroup heading="商品">
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
                        {priceText(it)}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              )}
              {res.authors.length > 0 && (
                <CommandGroup heading="作者">
                  {res.authors.map((a) => (
                    <CommandItem
                      key={a.name}
                      value={`s:${a.name}`}
                      onSelect={() => run(() => onAuthor(a.name))}
                    >
                      {a.image_url ? (
                        <img
                          src={a.image_url}
                          alt=""
                          className="size-8 rounded-full object-cover"
                        />
                      ) : (
                        <Store />
                      )}
                      <span className="flex-1 truncate">{a.name}</span>
                      <span className="text-xs text-muted-foreground">
                        {a.item_count} 个商品
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              )}
              {res.tags.length > 0 && (
                <CommandGroup heading="标签">
                  {res.tags.map((t) => (
                    <CommandItem
                      key={t.name}
                      value={`t:${t.name}`}
                      onSelect={() => run(() => onQuery(t.display))}
                    >
                      <Hash />
                      <span className="flex-1 truncate">{t.display}</span>
                      <span className="text-xs text-muted-foreground">
                        {t.item_count.toLocaleString()}
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
