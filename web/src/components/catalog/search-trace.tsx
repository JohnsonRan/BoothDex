"use client";

import { Activity } from "lucide-react";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import type { SearchResult } from "@/lib/catalog";
import type { MessageKey } from "@/lib/messages";
import { useSettings } from "@/lib/settings";

export type TraceMeta = Pick<SearchResult, "semantic" | "trace" | "timing"> & {
  /** request time measured in the browser */
  clientMs?: number;
};

const STEPS = new Set([
  "favourites",
  "settings",
  "embed_memory",
  "embed_db",
  "embed_provider",
  "embed_timeout",
  "embed_failed",
  "neighbours",
  "search",
  "items",
]);

/** How the search was answered (words, meaning, cache) and where the time went. */
export function SearchTrace({ meta }: { meta: TraceMeta }) {
  const { t } = useSettings();
  const steps = meta.trace ?? [];
  const cached = steps.some(
    (s) => s.step === "embed_memory" || s.step === "embed_db",
  );
  const mode =
    meta.semantic === "used"
      ? cached
        ? "trace.mode.cached"
        : "trace.mode.semantic"
      : meta.semantic === "unavailable"
        ? "trace.mode.unavailable"
        : "trace.mode.words";
  const server = meta.timing?.totalMs ?? 0;
  const ms = (n: number) => `${Math.round(n)} ms`;
  let sum = 0;

  return (
    <Popover>
      <PopoverTrigger
        className="inline-flex shrink-0 items-center gap-1 rounded-full border px-2 py-0.5 text-muted-foreground text-xs tabular-nums hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring"
        aria-label={`${t(mode)}, ${ms(meta.clientMs ?? server)}. ${t("trace.title")}`}
      >
        <Activity className="size-3" aria-hidden />
        {t(mode)} · {ms(meta.clientMs ?? server)}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80 p-0 text-sm">
        <p className="border-b px-4 py-2 font-semibold text-muted-foreground text-xs uppercase tracking-wide">
          {t("trace.title")}
        </p>
        <table className="w-full tabular-nums">
          <tbody>
            {steps.map((s, i) => {
              sum += s.ms;
              const key = (
                STEPS.has(s.step) ? `trace.step.${s.step}` : null
              ) as MessageKey | null;
              return (
                <tr
                  // steps can repeat in principle; the position keeps keys unique
                  // biome-ignore lint/suspicious/noArrayIndexKey: steps have no id
                  key={i}
                  className={
                    s.step === "embed_timeout" || s.step === "embed_failed"
                      ? "text-destructive"
                      : undefined
                  }
                >
                  <td className="px-4 py-1">{key ? t(key) : s.step}</td>
                  <td className="py-1 text-right">+{ms(s.ms)}</td>
                  <td className="px-4 py-1 text-right text-muted-foreground">
                    {ms(sum)}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        <p className="flex justify-between border-t px-4 py-2 text-muted-foreground text-xs">
          <span>
            {t("trace.server")} {ms(server)}
          </span>
          {meta.clientMs != null && (
            <span>
              {t("trace.total")} {ms(meta.clientMs)}
            </span>
          )}
        </p>
      </PopoverContent>
    </Popover>
  );
}
