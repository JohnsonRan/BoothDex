"use client";

import { Heart } from "lucide-react";
import { memo } from "react";
import {
  FEATURES,
  type Feature,
  type Item,
  imageSrc,
  imgUrl,
  isNew,
  priceOf,
} from "@/lib/catalog";
import { useSettings } from "@/lib/settings";
import { cn } from "@/lib/utils";

type Props = {
  item: Item;
  favourite: boolean;
  onOpen: (it: Item) => void;
  onFavourite: (it: Item) => void;
};

export const ItemCard = memo(function ItemCard({
  item,
  favourite,
  onOpen,
  onFavourite,
}: Props) {
  const { t, price, text } = useSettings();
  const second = imgUrl(item, 1);
  const free = priceOf(item) === 0;
  return (
    <article className="group relative">
      <button
        type="button"
        onClick={() => onOpen(item)}
        className="block w-full text-left"
        aria-label={item.title}
      >
        <div className="relative aspect-square overflow-hidden rounded-md bg-muted ring-1 ring-foreground/5">
          <img
            src={imgUrl(item, 0)}
            alt=""
            loading="lazy"
            decoding="async"
            className="absolute inset-0 size-full object-cover transition-transform duration-500 [@media(hover:hover)]:group-hover:scale-[1.04]"
          />
          {/* second image only on devices that can hover */}
          {second && (
            <img
              src={second}
              alt=""
              loading="lazy"
              decoding="async"
              className="absolute inset-0 hidden size-full object-cover opacity-0 transition-opacity duration-300 [@media(hover:hover)]:block [@media(hover:hover)]:group-hover:opacity-100"
            />
          )}
          <div className="absolute top-2 left-2 flex flex-wrap gap-1 pr-10">
            {isNew(item) && (
              <span className="rounded-sm bg-primary px-1.5 py-0.5 text-[10px] font-bold tracking-wider text-primary-foreground">
                NEW
              </span>
            )}
            {item.features.slice(0, 2).map((f) => (
              <span
                key={f}
                className="rounded-sm bg-background/75 px-1.5 py-0.5 text-[10px] font-semibold backdrop-blur"
              >
                {FEATURES[f as Feature] ?? f}
              </span>
            ))}
          </div>
        </div>
        <div className="space-y-1.5 px-0.5 pt-2 pb-1">
          <h3 className="line-clamp-2 min-h-[2.8em] text-[13px] leading-[1.4] transition-colors [@media(hover:hover)]:group-hover:text-primary">
            {text(item, "title")}
          </h3>
          <div className="flex items-center justify-between gap-2">
            <div className="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
              {item.author_icon && (
                <img
                  src={imageSrc(item.author_icon)}
                  alt=""
                  loading="lazy"
                  className="size-4 shrink-0 rounded-full bg-muted"
                />
              )}
              <span className="truncate">{item.author_name}</span>
            </div>
            <span
              className={cn(
                "shrink-0 text-xs font-bold tabular-nums",
                free ? "text-[var(--success)]" : "text-primary",
              )}
            >
              {price(item)}
            </span>
          </div>
        </div>
      </button>
      {/* always visible on touch screens; revealed on hover where hover exists */}
      <button
        type="button"
        aria-label={favourite ? t("fav.remove") : t("fav.add")}
        aria-pressed={favourite}
        onClick={() => onFavourite(item)}
        className={cn(
          "absolute top-2 right-2 grid size-8 place-items-center rounded-full bg-background/70 backdrop-blur transition-opacity",
          favourite
            ? "text-primary"
            : "text-foreground [@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-hover:opacity-100 focus-visible:opacity-100",
        )}
      >
        <Heart className={cn("size-4", favourite && "fill-current")} />
      </button>
    </article>
  );
});
