"use client";

import { Check } from "lucide-react";
import {
  type Category,
  FEATURES,
  type Feature,
  type Filters,
} from "@/lib/catalog";
import { useSettings } from "@/lib/settings";
import { cn } from "@/lib/utils";

type Props = {
  filters: Filters;
  categories: Category[];
  smartCounts: Record<string, number>;
  featureCounts: Record<string, number>;
  onChange: (patch: Partial<Filters>) => void;
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
  return (
    <div className="space-y-6 text-sm">
      <section>
        <Heading>{t("filter.content")}</Heading>
        <fieldset
          className="grid grid-cols-2 overflow-hidden rounded-lg border"
          aria-label={t("filter.rating")}
        >
          {(
            [
              ["safe", t("filter.safe")],
              ["include", t("filter.all")],
            ] as const
          ).map(([v, label]) => (
            <button
              key={v}
              type="button"
              aria-pressed={filters.adult === v}
              onClick={() => onChange({ adult: v })}
              className={cn(
                "py-2 text-xs transition",
                filters.adult === v
                  ? "bg-accent text-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {label}
            </button>
          ))}
        </fieldset>
      </section>

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
                  <span>
                    {(ui === "ja" ? c.ja : ui === "ko" ? c.ko : c.en) ?? c.en}
                  </span>
                  <span className="text-xs text-muted-foreground tabular-nums">
                    {n(smartCounts[c.slug])}
                  </span>
                </button>
              );
            })}
        </div>
      </section>

      <section>
        <Heading>{t("filter.features")}</Heading>
        <div className="space-y-0.5">
          {(Object.keys(FEATURES) as Feature[]).map((f) => {
            const on = filters.features.includes(f);
            return (
              <button
                key={f}
                type="button"
                aria-pressed={on}
                onClick={() => toggleFeature(f)}
                className="flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-left transition hover:bg-accent/60 lg:py-1.5"
              >
                <span
                  className={cn(
                    "grid size-4 place-items-center rounded border-[1.5px]",
                    on
                      ? "border-primary bg-primary text-primary-foreground"
                      : "border-muted-foreground",
                  )}
                >
                  {on && <Check className="size-3" strokeWidth={3} />}
                </span>
                <span className="flex-1">{FEATURES[f]}</span>
                <span className="text-xs text-muted-foreground tabular-nums">
                  {n(featureCounts[f])}
                </span>
              </button>
            );
          })}
        </div>
      </section>
    </div>
  );
}

function Heading({ children }: { children: React.ReactNode }) {
  return (
    <h4 className="mb-2 px-1 text-[11px] font-semibold tracking-widest text-muted-foreground uppercase">
      {children}
    </h4>
  );
}

/** Number of active filters, for the mobile filter button badge. */
export const activeFilterCount = (f: Filters) =>
  (f.sc ? 1 : 0) + f.features.length + (f.adult !== "safe" ? 1 : 0);
