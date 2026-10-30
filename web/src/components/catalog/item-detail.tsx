"use client";

import {
  ChevronLeft,
  ChevronRight,
  Copy,
  Download,
  ExternalLink,
  Heart,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { api } from "@/lib/api";
import {
  type Avatar,
  avatarName,
  FEATURES,
  type Feature,
  type Item,
  imageCount,
  imgUrl,
  priceOf,
  type SearchResult,
} from "@/lib/catalog";
import { type ContentLang, useSettings } from "@/lib/settings";
import { cn } from "@/lib/utils";

type Lang = ContentLang;
const LANGS: [Lang, string][] = [
  ["original", ""], // label comes from t("detail.original")
  ["en", "EN"],
  ["ja", "日本語"],
  ["ko", "한국어"],
];
const RATING = {
  SFW: "text-[var(--success)]",
  Suggestive: "text-[var(--warning)]",
  NSFW: "text-destructive",
} as const;

type Props = {
  item: Item | null;
  myAvatar: Avatar | null;
  favourite: boolean;
  adult: string;
  onClose: () => void;
  onFavourite: (it: Item) => void;
  onPickAvatar: (slug: string) => void;
  onSearch: (patch: { q?: string; author?: string }) => void;
  onOpenItem: (it: Item) => void;
};

export function ItemDetail({ item, ...rest }: Props) {
  return (
    <Dialog open={!!item} onOpenChange={(o) => !o && rest.onClose()}>
      <DialogContent
        showCloseButton={false}
        className="flex h-dvh max-h-dvh w-screen max-w-none flex-col gap-0 overflow-hidden rounded-none p-0 sm:max-w-none lg:h-auto lg:max-h-[92dvh] lg:w-[min(1240px,calc(100vw-48px))] lg:rounded-2xl"
      >
        {item && <Body key={item.id} item={item} {...rest} />}
      </DialogContent>
    </Dialog>
  );
}

function Body({
  item: initial,
  myAvatar,
  favourite,
  adult,
  onClose,
  onFavourite,
  onPickAvatar,
  onSearch,
  onOpenItem,
}: Omit<Props, "item"> & { item: Item }) {
  const { t, price, money, ui, content } = useSettings();
  const locale = { zh: "zh-CN", en: "en-US", ja: "ja-JP", ko: "ko-KR" }[ui];
  const [item, setItem] = useState(initial);
  // starts in the visitor's preferred item-text language, falling back to the original
  const [lang, setLang] = useState<Lang>(
    content !== "original" && initial.translations?.[content]
      ? content
      : "original",
  );
  const [idx, setIdx] = useState(0);
  const [bioOpen, setBioOpen] = useState(false);
  const [zoom, setZoom] = useState(false);
  const [same, setSame] = useState<Item[] | null>(null);
  const strip = useRef<HTMLDivElement>(null);

  // listings only carry 4 Booth images; the API fetches the complete gallery on first open
  useEffect(() => {
    api<{ images: string[]; complete: boolean }>(
      `/api/items/${initial.id}/gallery`,
    )
      .then(
        (g) =>
          g.complete &&
          g.images.length &&
          setItem((it) => ({ ...it, booth_images: g.images, media: [] })),
      )
      .catch(() => {});
    api<SearchResult>(
      `/api/search?authors=${encodeURIComponent(initial.author_name)}&adult=${adult}&sort=likes&pageSize=13`,
    )
      .then((r) =>
        setSame(r.items.filter((x) => x.id !== initial.id).slice(0, 12)),
      )
      .catch(() => setSame([]));
  }, [initial.id, initial.author_name, adult]);

  const count = Math.max(imageCount(item), 1);
  const go = (i: number) => {
    const n = (i + count) % count;
    strip.current?.scrollTo({
      left: n * strip.current.clientWidth,
      behavior: "smooth",
    });
  };
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight") go(idx + 1);
      if (e.key === "ArrowLeft") go(idx - 1);
    };
    addEventListener("keydown", key);
    return () => removeEventListener("keydown", key);
  });

  const tr = (field: "title" | "bio") =>
    (lang === "original" ? item[field] : item.translations?.[lang]?.[field]) ||
    item[field];
  const varName = (name: string) =>
    (lang === "original"
      ? name
      : item.translations?.[lang]?.variations?.[name]) || name;
  const showWhy =
    item.smart_price != null && item.smart_price !== item.price_jpy;
  const fits =
    !!myAvatar && item.linked_avatars.some((a) => a.slug === myAvatar.slug);
  const boothUrl = `https://booth.pm/ja/items/${item.id}`;
  const rating = item.content_rating
    ? ([
        t(`detail.rating.${item.content_rating}`),
        RATING[item.content_rating],
      ] as const)
    : null;

  return (
    <>
      <DialogTitle className="sr-only">{item.title}</DialogTitle>
      <DialogDescription className="sr-only">
        {t("detail.label")}
      </DialogDescription>
      <button
        type="button"
        onClick={onClose}
        aria-label={t("detail.close")}
        className="absolute top-3 right-3 z-20 grid size-9 place-items-center rounded-full border bg-background/80 backdrop-blur hover:bg-accent"
      >
        <X className="size-4" />
      </button>

      <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain">
        <div className="lg:grid lg:grid-cols-[minmax(0,1.2fr)_minmax(380px,1fr)]">
          {/* gallery: swipe on touch, arrows/keys/thumbnails on desktop */}
          <div className="lg:border-r lg:p-5">
            <div className="relative bg-muted lg:overflow-hidden lg:rounded-xl">
              <div
                ref={strip}
                onScroll={(e) =>
                  setIdx(
                    Math.round(
                      e.currentTarget.scrollLeft / e.currentTarget.clientWidth,
                    ),
                  )
                }
                className="flex aspect-square snap-x snap-mandatory overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
              >
                {Array.from({ length: count }, (_, i) => (
                  // biome-ignore lint/a11y/useKeyWithClickEvents: keyboard users page with arrow keys
                  <img
                    key={imgUrl(item, i, "620x620")}
                    src={imgUrl(item, i, "620x620")}
                    alt={t("detail.image", { n: i + 1 })}
                    loading={i < 2 ? "eager" : "lazy"}
                    onClick={() => setZoom(true)}
                    className="size-full shrink-0 snap-center object-contain lg:cursor-zoom-in"
                  />
                ))}
              </div>
              <DownloadMenu item={item} index={idx} />
              {count > 1 && (
                <>
                  <NavButton side="l" onClick={() => go(idx - 1)} />
                  <NavButton side="r" onClick={() => go(idx + 1)} />
                  <span className="absolute right-3 bottom-3 rounded-full bg-black/60 px-2.5 py-0.5 text-xs text-white tabular-nums">
                    {idx + 1} / {count}
                  </span>
                </>
              )}
            </div>
            {count > 1 && (
              <div className="mt-3 hidden max-h-40 grid-cols-[repeat(auto-fill,minmax(64px,1fr))] gap-2 overflow-y-auto lg:grid">
                {Array.from({ length: count }, (_, i) => (
                  <button
                    key={imgUrl(item, i, "72x72_a2_g5")}
                    type="button"
                    onClick={() => go(i)}
                    aria-label={t("detail.image", { n: i + 1 })}
                    className={cn(
                      "aspect-square overflow-hidden rounded-lg border-2 bg-muted",
                      i === idx
                        ? "border-primary"
                        : "border-transparent opacity-60 hover:opacity-100",
                    )}
                  >
                    <img
                      src={imgUrl(item, i, "72x72_a2_g5")}
                      alt=""
                      loading="lazy"
                      className="size-full object-cover"
                    />
                  </button>
                ))}
              </div>
            )}
          </div>

          <div className="space-y-4 p-4 lg:p-6">
            <div className="flex items-center gap-2.5 pr-10">
              {item.author_icon && (
                <img
                  src={item.author_icon}
                  alt=""
                  className="size-8 rounded-full bg-muted"
                />
              )}
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-medium">
                  {item.author_name}
                </div>
                <div className="truncate text-xs text-muted-foreground">
                  {safeHost(item.author_uri)}
                </div>
              </div>
              <button
                type="button"
                onClick={() => onSearch({ author: item.author_name })}
                className="shrink-0 rounded-full border px-3 py-1 text-xs hover:bg-accent"
              >
                {t("detail.shopAll")}
              </button>
            </div>

            <div
              className="inline-flex overflow-hidden rounded-lg border text-xs"
              role="tablist"
              aria-label={t("detail.language")}
            >
              {LANGS.filter(
                ([k]) => k === "original" || item.translations?.[k],
              ).map(([k, label]) => (
                <button
                  key={k}
                  type="button"
                  role="tab"
                  aria-selected={lang === k}
                  onClick={() => setLang(k)}
                  className={cn(
                    "px-2.5 py-1",
                    lang === k
                      ? "bg-accent text-foreground"
                      : "text-muted-foreground",
                  )}
                >
                  {k === "original" ? t("detail.original") : label}
                </button>
              ))}
            </div>

            <h2 className="text-lg leading-snug font-semibold tracking-tight lg:text-xl">
              {tr("title")}
            </h2>

            <div className="flex flex-wrap items-end gap-3">
              <span className="text-2xl leading-none font-extrabold text-primary tabular-nums lg:text-3xl">
                {price(item)}
              </span>
              {showWhy && (
                <span
                  className="cursor-help border-b border-dashed border-muted-foreground text-xs text-muted-foreground"
                  title={item.smart_price_reason ?? ""}
                >
                  {t("price.listed", { price: money(item.price_jpy) })}
                </span>
              )}
              <span className="ml-auto flex items-center gap-1 text-sm text-muted-foreground">
                <Heart className="size-3.5" />
                {item.likes.toLocaleString(locale)}
              </span>
            </div>

            <div className="flex flex-wrap gap-1.5 text-xs">
              {rating && (
                <span
                  className={cn(
                    "rounded-full border bg-muted px-2.5 py-0.5",
                    rating[1],
                  )}
                >
                  {rating[0]}
                </span>
              )}
              {item.features.map((f) => (
                <span
                  key={f}
                  className="rounded-full border bg-muted px-2.5 py-0.5"
                >
                  {FEATURES[f as Feature] ?? f}
                </span>
              ))}
              {item.quest === "Yes" && (
                <span className="rounded-full border bg-muted px-2.5 py-0.5">
                  {t("detail.quest")}
                </span>
              )}
              {item.status === "delisted" && (
                <span className="rounded-full border bg-muted px-2.5 py-0.5 text-destructive">
                  {t("detail.delisted")}
                </span>
              )}
            </div>

            {myAvatar && (
              <div
                className={cn(
                  "flex items-center gap-2.5 rounded-xl border px-3 py-2.5 text-sm font-medium",
                  fits
                    ? "border-[var(--success)]/30 bg-[var(--success)]/10 text-[var(--success)]"
                    : "border-[var(--warning)]/30 bg-[var(--warning)]/8 text-[var(--warning)]",
                )}
              >
                {myAvatar.image_url && (
                  <img
                    src={myAvatar.image_url}
                    alt=""
                    className="size-8 rounded-lg object-cover"
                  />
                )}
                {fits
                  ? t("detail.fits", { name: avatarName(myAvatar, ui) })
                  : item.needs_enrichment
                    ? t("detail.notAnalysed", {
                        name: avatarName(myAvatar, ui),
                      })
                    : t("detail.notListed", { name: avatarName(myAvatar, ui) })}
              </div>
            )}

            {/* desktop actions; phones get the sticky bar below */}
            <div className="hidden gap-2 lg:flex">
              <a
                href={boothUrl}
                target="_blank"
                rel="noopener"
                className="flex flex-[2] items-center justify-center gap-1.5 rounded-lg bg-brand py-2.5 text-sm font-semibold text-white"
              >
                {t("detail.buy")} <ExternalLink className="size-3.5" />
              </a>
              <FavButton on={favourite} onClick={() => onFavourite(item)} />
              <CopyButton url={boothUrl} />
            </div>

            {item.variations.length > 0 && (
              <Section
                title={t("detail.variations", { n: item.variations.length })}
              >
                <div className="divide-y overflow-hidden rounded-lg border text-sm">
                  {item.variations.map((v) => (
                    <div
                      key={v.id}
                      className={cn(
                        "flex justify-between gap-3 px-3 py-2",
                        v.status !== "addable_to_cart" &&
                          "text-muted-foreground line-through",
                      )}
                    >
                      <span className="min-w-0">
                        {varName(v.name) || t("detail.defaultVariation")}
                      </span>
                      <b className="shrink-0 tabular-nums">
                        {money(v.price_jpy ?? 0)}
                      </b>
                    </div>
                  ))}
                </div>
              </Section>
            )}

            {item.linked_avatars.length > 0 && (
              <Section
                title={t("detail.avatars", { n: item.linked_avatars.length })}
              >
                <div className="flex flex-wrap gap-1.5">
                  {item.linked_avatars.map((a) => (
                    <button
                      key={a.slug}
                      type="button"
                      onClick={() => onPickAvatar(a.slug)}
                      className={cn(
                        "flex items-center gap-1.5 rounded-full border bg-muted py-0.5 pr-2.5 pl-0.5 text-xs hover:bg-accent",
                        a.slug === myAvatar?.slug &&
                          "border-[var(--success)] text-[var(--success)]",
                      )}
                    >
                      {a.image_url && (
                        <img
                          src={a.image_url}
                          alt=""
                          loading="lazy"
                          className="size-6 rounded-full object-cover"
                        />
                      )}
                      {avatarName(a, ui)}
                    </button>
                  ))}
                </div>
              </Section>
            )}
          </div>
        </div>

        <div className="border-t lg:grid lg:grid-cols-[minmax(0,1.2fr)_minmax(380px,1fr)]">
          <div className="p-4 lg:border-r lg:p-6">
            <Section title={t("detail.bio")}>
              <Bio
                text={tr("bio") ?? ""}
                open={bioOpen}
                onToggle={() => setBioOpen((o) => !o)}
              />
            </Section>
          </div>
          <div className="space-y-5 border-t p-4 lg:border-t-0 lg:p-6">
            {item.booth_tags.length > 0 && (
              <Section title={t("detail.tags")}>
                <div className="flex flex-wrap gap-1.5">
                  {item.booth_tags.slice(0, 40).map((t) => (
                    <button
                      key={t}
                      type="button"
                      onClick={() => onSearch({ q: t })}
                      className="rounded-md border px-2 py-0.5 text-xs text-muted-foreground hover:text-foreground"
                    >
                      {t}
                    </button>
                  ))}
                </div>
              </Section>
            )}
            <Section title={t("detail.info")}>
              <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-xs text-muted-foreground">
                <dt>{t("detail.itemId")}</dt>
                <dd className="text-foreground">{item.id}</dd>
                <dt>{t("detail.published")}</dt>
                <dd className="text-foreground">
                  {item.published_at
                    ? new Date(item.published_at).toLocaleDateString(locale)
                    : "—"}
                </dd>
                <dt>{t("detail.boothCategory")}</dt>
                <dd className="text-foreground">
                  {item.booth_category ?? "—"}
                </dd>
                <dt>{t("detail.scraped")}</dt>
                <dd className="text-foreground">
                  {new Date(item.scraped_at).toLocaleString(locale)}
                </dd>
              </dl>
            </Section>
          </div>
        </div>

        <div className="border-t p-4 pb-24 lg:p-6">
          <Section title={t("detail.moreFrom", { name: item.author_name })}>
            <div className="grid auto-cols-[132px] grid-flow-col gap-3 overflow-x-auto pb-1 lg:auto-cols-[150px]">
              {same === null && (
                <span className="text-sm text-muted-foreground">
                  {t("detail.loading")}
                </span>
              )}
              {same?.length === 0 && (
                <span className="text-sm text-muted-foreground">
                  {t("detail.noMore")}
                </span>
              )}
              {same?.map((x) => (
                <button
                  key={x.id}
                  type="button"
                  onClick={() => onOpenItem(x)}
                  className="text-left"
                >
                  <img
                    src={imgUrl(x, 0)}
                    alt=""
                    loading="lazy"
                    className="aspect-square w-full rounded-lg bg-muted object-cover"
                  />
                  <div className="mt-1.5 line-clamp-2 text-xs">{x.title}</div>
                  <b className="text-xs text-primary">{price(x)}</b>
                </button>
              ))}
            </div>
          </Section>
        </div>
      </div>

      {/* phone action bar */}
      <div className="flex items-center gap-2 border-t bg-popover/95 p-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] backdrop-blur lg:hidden">
        <span
          className={cn(
            "mr-1 text-lg font-extrabold tabular-nums",
            priceOf(item) === 0 ? "text-[var(--success)]" : "text-primary",
          )}
        >
          {price(item)}
        </span>
        <FavButton on={favourite} onClick={() => onFavourite(item)} compact />
        <a
          href={boothUrl}
          target="_blank"
          rel="noopener"
          className="flex flex-1 items-center justify-center gap-1.5 rounded-lg bg-brand py-2.5 text-sm font-semibold text-white"
        >
          {t("detail.buy")} <ExternalLink className="size-3.5" />
        </a>
      </div>

      {zoom && (
        <button
          type="button"
          aria-label={t("detail.closeZoom")}
          onClick={() => setZoom(false)}
          className="fixed inset-0 z-50 hidden cursor-zoom-out place-items-center bg-black/90 lg:grid"
        >
          <img
            src={imgUrl(item, idx, "orig")}
            alt=""
            className="max-h-[94vh] max-w-[96vw] object-contain"
          />
        </button>
      )}
    </>
  );
}

function safeHost(u: string) {
  try {
    return new URL(u).host;
  } catch {
    return u;
  }
}

function NavButton({
  side,
  onClick,
}: {
  side: "l" | "r";
  onClick: () => void;
}) {
  const { t } = useSettings();
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={side === "l" ? t("detail.prev") : t("detail.next")}
      className={cn(
        "absolute top-1/2 hidden size-10 -translate-y-1/2 place-items-center rounded-full bg-black/50 text-white hover:bg-black/70 lg:grid",
        side === "l" ? "left-3" : "right-3",
      )}
    >
      {side === "l" ? (
        <ChevronLeft className="size-5" />
      ) : (
        <ChevronRight className="size-5" />
      )}
    </button>
  );
}

function FavButton({
  on,
  onClick,
  compact,
}: {
  on: boolean;
  onClick: () => void;
  compact?: boolean;
}) {
  const { t } = useSettings();
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={on}
      aria-label={on ? t("fav.remove") : t("fav.add")}
      className={cn(
        "flex items-center justify-center gap-1.5 rounded-lg border px-3 py-2.5 text-sm font-medium hover:bg-accent",
        !compact && "flex-1",
        on && "border-primary text-primary",
      )}
    >
      <Heart className={cn("size-4", on && "fill-current")} />
      {!compact && (on ? t("fav.added") : t("fav.add"))}
    </button>
  );
}

/** Saves the current image at full size, re-encoded by the image proxy. Archived (delisted) images are WebP only. */
function DownloadMenu({ item, index }: { item: Item; index: number }) {
  const { t } = useSettings();
  const orig = imgUrl(item, index, "orig");
  if (!orig) return null;
  const archived = orig.startsWith("/img/v/");
  const formats = archived
    ? (["webp"] as const)
    : (["webp", "png", "jpeg"] as const);
  const save = async (format: string) => {
    try {
      const res = await fetch(archived ? orig : `${orig}?format=${format}`);
      if (!res.ok) throw new Error(String(res.status));
      const a = document.createElement("a");
      a.href = URL.createObjectURL(await res.blob());
      a.download = `booth-${item.id}-${index + 1}.${format === "jpeg" ? "jpg" : format}`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 10_000);
    } catch {
      toast.error(t("list.loadFailed"));
    }
  };
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={t("detail.download")}
          className="absolute bottom-3 left-3 z-10 grid size-9 place-items-center rounded-full bg-black/55 text-white hover:bg-black/75"
        >
          <Download className="size-4" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuLabel>{t("detail.download")}</DropdownMenuLabel>
        {formats.map((f) => (
          <DropdownMenuItem key={f} onSelect={() => save(f)}>
            {t("detail.downloadAs", { format: f.toUpperCase() })}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function CopyButton({ url }: { url: string }) {
  const { t } = useSettings();
  return (
    <button
      type="button"
      onClick={() =>
        navigator.clipboard
          .writeText(url)
          .then(() => toast.success(t("detail.copied")))
      }
      className="flex flex-1 items-center justify-center gap-1.5 rounded-lg border px-3 py-2.5 text-sm font-medium hover:bg-accent"
    >
      <Copy className="size-4" />
      {t("detail.copy")}
    </button>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <h3 className="mb-2 text-[11px] font-semibold tracking-widest text-muted-foreground uppercase">
        {title}
      </h3>
      {children}
    </section>
  );
}

function Bio({
  text,
  open,
  onToggle,
}: {
  text: string;
  open: boolean;
  onToggle: () => void;
}) {
  const { t } = useSettings();
  const ref = useRef<HTMLDivElement>(null);
  const [long, setLong] = useState(false);
  useEffect(() => {
    if (ref.current)
      setLong(ref.current.scrollHeight > ref.current.clientHeight + 4);
  }, []);
  return (
    <>
      <div
        ref={ref}
        className={cn(
          "relative text-[13.5px] leading-7 whitespace-pre-wrap text-foreground/80",
          !open && "max-h-80 overflow-hidden",
          !open &&
            long &&
            "after:absolute after:inset-x-0 after:bottom-0 after:h-20 after:bg-gradient-to-b after:from-transparent after:to-popover",
        )}
      >
        {text}
      </div>
      {long && (
        <button
          type="button"
          onClick={onToggle}
          className="mt-2 text-sm text-primary"
        >
          {open ? t("detail.less") : t("detail.more")}
        </button>
      )}
    </>
  );
}
