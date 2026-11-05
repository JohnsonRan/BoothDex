"use client";
// Quick search: Astryx CommandPalette with an async source (block CommandPaletteAsyncSearch), loaded on first
// open. Master's features: recent searches when the query is empty, then the query itself with its result count,
// avatars, items, shops and tags from /api/palette (120 ms debounce). The open palette is one history entry, so
// a phone's Back closes it instead of leaving the site.

import { Avatar } from "@astryxdesign/core/Avatar";
import {
  CommandPalette,
  CommandPaletteInput,
} from "@astryxdesign/core/CommandPalette";
import { Icon } from "@astryxdesign/core/Icon";
import { Text } from "@astryxdesign/core/Text";
import { Hash, Trash2 } from "lucide-react";
import { use, useEffect, useMemo, useRef } from "react";
import { useUi } from "@/components/providers";
import { saveHistory } from "@/lib/history";
import type { Rates } from "@/lib/settings-core";
import { type Pick, paletteSource, type Row } from "./palette-source";

// the theme's semantic icons where it has one (TEMPLATE-MAP rule 6), lucide otherwise
const ICONS = {
  history: "clock",
  search: "search",
  tag: Hash,
  clear: Trash2,
} as const;

export default function Palette({
  isOpen,
  onOpenChange,
  adult,
  rates,
  onPick,
}: {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  adult: string;
  rates: Promise<Rates>;
  onPick: (pick: Pick) => void;
}) {
  const { t, ui, settings } = useUi();
  const known = use(rates);
  const rows = useRef(new Map<string, Row>());

  const source = useMemo(
    () =>
      paletteSource({
        t,
        ui,
        settings,
        rates: known,
        adult,
        rows: rows.current,
      }),
    [adult, known, settings, t, ui],
  );

  // Open = one history entry, so Back closes the palette. Closing without a pick steps back; a pick takes the
  // entry off first, then navigates, so no duplicate entry is left behind.
  const entry = useRef(false);
  const close = useRef(onOpenChange);
  close.current = onOpenChange;
  useEffect(() => {
    if (isOpen && !entry.current) {
      entry.current = true;
      window.history.pushState(window.history.state, "", location.href);
    } else if (!isOpen && entry.current) {
      entry.current = false;
      window.history.back();
    }
    if (!isOpen) return;
    const onBack = () => {
      if (!entry.current) return;
      entry.current = false;
      close.current(false);
    };
    addEventListener("popstate", onBack);
    return () => removeEventListener("popstate", onBack);
  }, [isOpen]);

  const choose = (id: string) => {
    const pick = rows.current.get(id)?.auxiliaryData?.pick;
    if (!pick) return;
    if (pick.kind === "clear") {
      saveHistory([]);
      return onOpenChange(false);
    }
    const run = () => onPick(pick);
    onOpenChange(false);
    if (entry.current) {
      entry.current = false;
      addEventListener("popstate", run, { once: true });
      window.history.back();
    } else run();
  };

  return (
    <CommandPalette<Row>
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      searchSource={source}
      label={t("palette.title")}
      input={
        <CommandPaletteInput
          label={t("palette.title")}
          placeholder={t("palette.placeholder")}
        />
      }
      emptyBootstrapText={t("palette.hint")}
      emptySearchText={t("palette.empty")}
      onValueChange={choose}
      renderItem={(row) => {
        const d = row.auxiliaryData;
        // a row as in block CommandPaletteRichItems: leading visual, label taking the width, trailing note
        return (
          <>
            {d?.image !== undefined ? (
              <Avatar
                src={d.image || undefined}
                name={row.label}
                size="sm"
                shape={d.square ? "rounded" : "circle"}
                tooltip={false}
              />
            ) : (
              d?.icon && (
                <Icon icon={ICONS[d.icon]} size="sm" color="secondary" />
              )
            )}
            <Text
              type="body"
              maxLines={1}
              hasTruncateTooltip={false}
              style={{ flex: 1 }}
            >
              {row.label}
              {d?.note && (
                <Text type="body" color="secondary">{` ${d.note}`}</Text>
              )}
            </Text>
            {d?.meta && (
              <Text type="supporting" color="secondary" hasTabularNumbers>
                {d.meta}
              </Text>
            )}
          </>
        );
      }}
    />
  );
}
