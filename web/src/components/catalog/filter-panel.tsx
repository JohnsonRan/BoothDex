"use client";

import { Check, Minus, Plus, X } from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import {
  type Category,
  FEATURES,
  type Feature,
  type Filters,
  localName,
  RANGES,
  type Range,
  tagName,
} from "@/lib/catalog";
import type { MessageKey } from "@/lib/messages";
import { useSettings } from "@/lib/settings";
import { cn } from "@/lib/utils";

type Props = {
  filters: Filters;
  categories: Category[];
  smartCounts: Record<string, number>;
  featureCounts: Record<string, number>;
  onChange: (patch: Partial<Filters>) => void;
};

type Tag = {
  name: string;
  display: string;
  display_zh?: string | null;
  item_count: number;
};

const n = (v: number | undefined) => (v ?? 0).toLocaleString();

export function FilterPanel({
  filters,
  categories,
  smartCounts,
  featureCounts,
  onChange,
}: Props) {
  const { t, ui } = useSettings();
  const toggleFeature = (f: Feature) =>
    onChange({
      features: filters.features.includes(f)
        ? filters.features.filter((x) => x !== f)
        : [...filters.features, f],
    });
  const catName = (c: Category) => localName(c, ui);
  const avatars = filters.view === "avatars";

  return (
    <div className="space-y-6 text-sm">
      {filters.view !== "favourites" && (
        <Segmented
          label={t("view.items")}
          value={filters.view}
          options={[
            ["items", t("view.items")],
            ["avatars", t("view.avatars")],
          ]}
          onChange={(v) =>
            onChange({ view: v as Filters["view"], avatar: "", sc: "" })
          }
        />
      )}

      <section>
        <Heading>{t("filter.content")}</Heading>
        <Segmented
          label={t("filter.rating")}
          value={filters.adult}
          options={[
            ["safe", t("filter.safe")],
            ["include", t("filter.all")],
          ]}
          onChange={(v) => onChange({ adult: v as Filters["adult"] })}
        />
      </section>

      {avatars && (
        <section className="space-y-2">
          <Heading>{t("filter.gender")}</Heading>
          <Segmented
            label={t("filter.gender")}
            value={filters.gender}
            options={[
              ["", t("filter.genderAny")],
              ["female", t("filter.female")],
              ["male", t("filter.male")],
            ]}
            onChange={(v) => onChange({ gender: v as Filters["gender"] })}
          />
          <CheckRow
            on={filters.asup === "supported"}
            label={t("filter.supportedOnly")}
            onClick={() =>
              onChange({
                asup: filters.asup === "supported" ? "all" : "supported",
              })
            }
          />
        </section>
      )}

      {!avatars && (
        <section>
          <Heading>{t("filter.categories")}</Heading>
          <div className="space-y-0.5">
            {categories
              .filter((c) => c.mode === "items")
              .map((c) => {
                const on = filters.sc === c.slug;
                return (
                  <button
                    key={c.slug}
                    type="button"
                    aria-pressed={on}
                    onClick={() => onChange({ sc: on ? "" : c.slug })}
                    className={cn(
                      "flex w-full items-center justify-between gap-2 rounded-md px-2.5 py-2 text-left transition hover:bg-accent/60 lg:py-1.5",
                      on && "bg-accent shadow-[inset_2px_0_0_var(--primary)]",
                    )}
                  >
                    <span>{catName(c)}</span>
                    <span className="text-xs text-muted-foreground tabular-nums">
                      {n(smartCounts[c.slug])}
                    </span>
                  </button>
                );
              })}
          </div>
        </section>
      )}

      <PriceSection filters={filters} onChange={onChange} />

      <section>
        <Heading>{t("filter.time")}</Heading>
        <div className="flex flex-wrap gap-1.5">
          {(["", ...RANGES] as Range[]).map((r) => (
            <Chip
              key={r || "any"}
              on={filters.range === r}
              onClick={() => onChange({ range: r })}
            >
              {r ? t(`range.${r}` as MessageKey) : t("filter.timeAny")}
            </Chip>
          ))}
        </div>
      </section>

      <section>
        <Heading>{t("filter.features")}</Heading>
        <div className="space-y-0.5">
          {(Object.keys(FEATURES) as Feature[]).map((f) => (
            <CheckRow
              key={f}
              on={filters.features.includes(f)}
              label={FEATURES[f]}
              count={n(featureCounts[f])}
              onClick={() => toggleFeature(f)}
            />
          ))}
        </div>
      </section>

      <TagSection filters={filters} onChange={onChange} />
    </div>
  );
}

function PriceSection({
  filters,
  onChange,
}: {
  filters: Filters;
  onChange: Props["onChange"];
}) {
  const { t } = useSettings();
  const [min, setMin] = useState(filters.pmin ? String(filters.pmin) : "");
  const [max, setMax] = useState(filters.pmax ? String(filters.pmax) : "");
  useEffect(() => {
    setMin(filters.pmin ? String(filters.pmin) : "");
    setMax(filters.pmax ? String(filters.pmax) : "");
  }, [filters.pmin, filters.pmax]);
  const apply = (e: FormEvent) => {
    e.preventDefault();
    const lo = Math.max(0, Number.parseInt(min, 10) || 0);
    const hi = Math.max(0, Number.parseInt(max, 10) || 0);
    onChange({
      free: false,
      pmin: hi && lo > hi ? hi : lo,
      pmax: hi && lo > hi ? lo : hi,
    });
  };
  // presets in yen, matching how Booth items are usually priced
  const presets: [number, number, string][] = [
    [0, 500, "≤ ¥500"],
    [500, 1500, "¥500–1,500"],
    [1500, 3000, "¥1,500–3,000"],
    [3000, 0, "¥3,000+"],
  ];
  return (
    <section className="space-y-2">
      <Heading>{t("filter.price")}</Heading>
      <CheckRow
        on={filters.free}
        label={t("filter.free")}
        onClick={() => onChange({ free: !filters.free })}
      />
      {!filters.free && (
        <>
          <div className="flex flex-wrap gap-1.5">
            {presets.map(([lo, hi, label]) => (
              <Chip
                key={label}
                on={filters.pmin === lo && filters.pmax === hi}
                onClick={() =>
                  onChange(
                    filters.pmin === lo && filters.pmax === hi
                      ? { pmin: 0, pmax: 0 }
                      : { pmin: lo, pmax: hi },
                  )
                }
              >
                {label}
              </Chip>
            ))}
          </div>
          <form onSubmit={apply} className="flex items-center gap-1.5">
            <Input
              inputMode="numeric"
              value={min}
              onChange={(e) => setMin(e.target.value.replace(/\D/g, ""))}
              placeholder={`¥ ${t("filter.min")}`}
              aria-label={t("filter.min")}
              className="h-8"
            />
            <span className="text-muted-foreground">–</span>
            <Input
              inputMode="numeric"
              value={max}
              onChange={(e) => setMax(e.target.value.replace(/\D/g, ""))}
              placeholder={`¥ ${t("filter.max")}`}
              aria-label={t("filter.max")}
              className="h-8"
            />
            <button
              type="submit"
              className="h-8 shrink-0 rounded-md border px-2.5 text-xs hover:bg-accent"
            >
              {t("filter.apply")}
            </button>
          </form>
        </>
      )}
    </section>
  );
}

/** Smart-tag picker: search, then include (+) or exclude (−). */
function TagSection({
  filters,
  onChange,
}: {
  filters: Filters;
  onChange: Props["onChange"];
}) {
  const { t, ui } = useSettings();
  const [q, setQ] = useState("");
  const [found, setFound] = useState<Tag[]>([]);
  const [labels, setLabels] = useState<Record<string, string>>({});

  useEffect(() => {
    const ctl = new AbortController();
    const h = setTimeout(() => {
      api<{ items: Tag[] }>(
        `/api/tags?q=${encodeURIComponent(q)}&limit=${q ? 8 : 6}`,
        { signal: ctl.signal },
      )
        .then((r) => {
          setFound(r.items);
          setLabels((l) => ({
            ...l,
            ...Object.fromEntries(r.items.map((x) => [x.name, tagName(x, ui)])),
          }));
        })
        .catch(() => {});
    }, 150);
    return () => {
      clearTimeout(h);
      ctl.abort();
    };
  }, [q, ui]);

  const set = (name: string, mode: "tags" | "extags" | null) =>
    onChange({
      tags:
        mode === "tags"
          ? [...new Set([...filters.tags, name])]
          : filters.tags.filter((x) => x !== name),
      extags:
        mode === "extags"
          ? [...new Set([...filters.extags, name])]
          : filters.extags.filter((x) => x !== name),
    });
  const chosen = [
    ...filters.tags.map((x) => [x, "tags"] as const),
    ...filters.extags.map((x) => [x, "extags"] as const),
  ];

  return (
    <section className="space-y-2">
      <Heading>{t("filter.tags")}</Heading>
      {chosen.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {chosen.map(([name, mode]) => (
            <button
              key={name}
              type="button"
              onClick={() => set(name, null)}
              aria-label={t("filter.removeTag", { name: labels[name] ?? name })}
              className={cn(
                "flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs",
                mode === "tags"
                  ? "border-[var(--success)]/40 text-[var(--success)]"
                  : "border-destructive/40 text-destructive line-through",
              )}
            >
              {labels[name] ?? name}
              <X className="size-3" />
            </button>
          ))}
        </div>
      )}
      <Input
        value={q}
        onChange={(e) => setQ(e.target.value)}
        placeholder={t("filter.tagSearch")}
        aria-label={t("filter.tagSearch")}
        className="h-8"
      />
      <div className="space-y-0.5">
        {found
          .filter(
            (x) =>
              !filters.tags.includes(x.name) &&
              !filters.extags.includes(x.name),
          )
          .map((x) => (
            <div
              key={x.name}
              className="flex items-center gap-1 rounded-md px-1.5 py-1 hover:bg-accent/40"
            >
              <span className="min-w-0 flex-1 truncate">{tagName(x, ui)}</span>
              <span className="text-xs text-muted-foreground tabular-nums">
                {n(x.item_count)}
              </span>
              <button
                type="button"
                onClick={() => set(x.name, "tags")}
                aria-label={`${t("filter.include")} ${tagName(x, ui)}`}
                className="grid size-6 place-items-center rounded text-[var(--success)] hover:bg-accent"
              >
                <Plus className="size-3.5" />
              </button>
              <button
                type="button"
                onClick={() => set(x.name, "extags")}
                aria-label={`${t("filter.exclude")} ${tagName(x, ui)}`}
                className="grid size-6 place-items-center rounded text-destructive hover:bg-accent"
              >
                <Minus className="size-3.5" />
              </button>
            </div>
          ))}
      </div>
    </section>
  );
}

function Segmented({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: [string, string][];
  onChange: (v: string) => void;
}) {
  return (
    <fieldset
      className="grid overflow-hidden rounded-lg border"
      style={{
        gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))`,
      }}
      aria-label={label}
    >
      {options.map(([v, text]) => (
        <button
          key={v || "any"}
          type="button"
          aria-pressed={value === v}
          onClick={() => onChange(v)}
          className={cn(
            "py-2 text-xs transition",
            value === v
              ? "bg-accent text-foreground"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          {text}
        </button>
      ))}
    </fieldset>
  );
}

function CheckRow({
  on,
  label,
  count,
  onClick,
}: {
  on: boolean;
  label: string;
  count?: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={on}
      onClick={onClick}
      className="flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-left transition hover:bg-accent/60 lg:py-1.5"
    >
      <span
        className={cn(
          "grid size-4 shrink-0 place-items-center rounded border-[1.5px]",
          on
            ? "border-primary bg-primary text-primary-foreground"
            : "border-muted-foreground",
        )}
      >
        {on && <Check className="size-3" strokeWidth={3} />}
      </span>
      <span className="flex-1">{label}</span>
      {count && (
        <span className="text-xs text-muted-foreground tabular-nums">
          {count}
        </span>
      )}
    </button>
  );
}

function Chip({
  on,
  onClick,
  children,
}: {
  on: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-pressed={on}
      onClick={onClick}
      className={cn(
        "rounded-full border px-2.5 py-1 text-xs transition",
        on
          ? "border-primary bg-primary/12 text-primary"
          : "text-muted-foreground hover:text-foreground",
      )}
    >
      {children}
    </button>
  );
}

function Heading({ children }: { children: React.ReactNode }) {
  return (
    <h2 className="mb-2 px-1 text-[11px] font-semibold tracking-widest text-muted-foreground uppercase">
      {children}
    </h2>
  );
}

/** Number of active filters, for the mobile filter button badge. */
export const activeFilterCount = (f: Filters) =>
  (f.sc ? 1 : 0) +
  f.features.length +
  (f.adult !== "safe" ? 1 : 0) +
  f.tags.length +
  f.extags.length +
  (f.free || f.pmin || f.pmax ? 1 : 0) +
  (f.range ? 1 : 0) +
  (f.view === "avatars" && f.gender ? 1 : 0) +
  (f.view === "avatars" && f.asup !== "supported" ? 1 : 0);
