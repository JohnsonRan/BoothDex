"use client";

import { ChevronLeft, ChevronRight, Copy, Download, Heart } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { type Item, imgUrl } from "@/lib/catalog";
import { useSettings } from "@/lib/settings";
import { cn } from "@/lib/utils";

export function safeHost(u: string) {
  try {
    return new URL(u).host;
  } catch {
    return u;
  }
}

export function NavButton({
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

export function FavButton({
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
export function DownloadMenu({ item, index }: { item: Item; index: number }) {
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

export function CopyButton({ url }: { url: string }) {
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

export function Section({
  title,
  action,
  children,
}: {
  title: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section>
      <div className="mb-2 flex items-center justify-between gap-2">
        <h3 className="text-[11px] font-semibold tracking-widest text-muted-foreground uppercase">
          {title}
        </h3>
        {action}
      </div>
      {children}
    </section>
  );
}

export function Bio({
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
