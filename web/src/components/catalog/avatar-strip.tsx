"use client";

import { ChevronLeft, ChevronRight, Sparkles } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { type Avatar, avatarName } from "@/lib/catalog";
import { useSettings } from "@/lib/settings";
import { cn } from "@/lib/utils";

/** Horizontal avatar picker: swipe on touch, arrows + mouse wheel on desktop, no visible scrollbar. */
export function AvatarStrip({
  avatars,
  value,
  onChange,
}: {
  avatars: Avatar[];
  value: string;
  onChange: (slug: string) => void;
}) {
  const { t, ui } = useSettings();
  const ref = useRef<HTMLDivElement>(null);
  const [edges, setEdges] = useState({ l: false, r: false });

  const update = useCallback(() => {
    const el = ref.current;
    if (el)
      setEdges({
        l: el.scrollLeft > 4,
        r: el.scrollLeft + el.clientWidth < el.scrollWidth - 4,
      });
  }, []);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    update();
    const onWheel = (e: WheelEvent) => {
      if (
        Math.abs(e.deltaY) <= Math.abs(e.deltaX) ||
        el.scrollWidth <= el.clientWidth
      )
        return;
      e.preventDefault();
      el.scrollLeft += e.deltaY;
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => {
      el.removeEventListener("wheel", onWheel);
      ro.disconnect();
    };
  }, [update]);

  // keep the selected avatar in view (e.g. picked from the palette)
  useEffect(() => {
    const el = ref.current?.querySelector<HTMLElement>(
      `[data-slug="${CSS.escape(value)}"]`,
    );
    if (el && ref.current) {
      const s = ref.current;
      if (
        el.offsetLeft < s.scrollLeft ||
        el.offsetLeft + el.offsetWidth > s.scrollLeft + s.clientWidth
      )
        s.scrollTo({ left: el.offsetLeft - 48, behavior: "smooth" });
    }
  }, [value]);

  const page = (dir: 1 | -1) =>
    ref.current?.scrollBy({
      left: dir * ref.current.clientWidth * 0.8,
      behavior: "smooth",
    });
  const mask =
    edges.l && edges.r
      ? "[mask-image:linear-gradient(90deg,transparent,#000_64px,#000_calc(100%-64px),transparent)]"
      : edges.l
        ? "[mask-image:linear-gradient(90deg,transparent,#000_64px)]"
        : edges.r
          ? "[mask-image:linear-gradient(90deg,#000_calc(100%-64px),transparent)]"
          : "";

  return (
    <div className="relative">
      <div
        ref={ref}
        onScroll={update}
        className={cn(
          "flex snap-x gap-2.5 overflow-x-auto py-3 [scrollbar-width:none] sm:gap-3 [&::-webkit-scrollbar]:hidden",
          mask,
        )}
      >
        <Tile slug="" active={!value} label={t("strip.all")} onClick={onChange}>
          <div className="grid size-full place-items-center bg-muted text-muted-foreground">
            <Sparkles className="size-5" />
          </div>
        </Tile>
        {avatars.map((a) => (
          <Tile
            key={a.slug}
            slug={a.slug}
            active={a.slug === value}
            label={avatarName(a, ui)}
            onClick={onChange}
          >
            {a.image_url && (
              <img
                src={a.image_url}
                alt=""
                loading="lazy"
                className="size-full object-cover"
              />
            )}
          </Tile>
        ))}
      </div>
      {(["l", "r"] as const).map((side) => (
        <button
          key={side}
          type="button"
          aria-label={side === "l" ? t("strip.left") : t("strip.right")}
          onClick={() => page(side === "l" ? -1 : 1)}
          className={cn(
            "absolute top-1/2 z-10 hidden size-9 -translate-y-[60%] place-items-center rounded-full border bg-popover/90 shadow-lg backdrop-blur transition-opacity hover:bg-accent [@media(hover:hover)]:grid",
            side === "l" ? "left-0" : "right-0",
            edges[side] ? "opacity-100" : "pointer-events-none opacity-0",
          )}
        >
          {side === "l" ? (
            <ChevronLeft className="size-5" />
          ) : (
            <ChevronRight className="size-5" />
          )}
        </button>
      ))}
    </div>
  );
}

function Tile({
  slug,
  active,
  label,
  onClick,
  children,
}: {
  slug: string;
  active: boolean;
  label: string;
  onClick: (s: string) => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      data-slug={slug}
      onClick={() => onClick(slug)}
      aria-pressed={active}
      className="w-[76px] shrink-0 snap-start text-center sm:w-[104px]"
    >
      <div
        className={cn(
          "mb-1.5 h-24 overflow-hidden rounded-xl border-2 transition sm:h-[132px] sm:rounded-2xl",
          active
            ? "border-primary shadow-[0_8px_24px_-6px_var(--primary)]"
            : "border-transparent brightness-[.82] saturate-[.85] [@media(hover:hover)]:hover:brightness-100",
        )}
      >
        {children}
      </div>
      <span
        className={cn(
          "block truncate text-[11px] sm:text-xs",
          active ? "font-semibold text-foreground" : "text-muted-foreground",
        )}
      >
        {label}
      </span>
    </button>
  );
}
