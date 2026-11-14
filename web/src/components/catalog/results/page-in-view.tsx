"use client";
// The results page in view (result-pages.tsx), for the readout beside the count: the results set it as the reader
// scrolls, and the readout watches it.

import { HStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useSyncExternalStore } from "react";
import { useUi } from "@/components/providers";

/** The page in view while the results are on screen (the one in the address), for the readout beside the count. */
let pageInView: number | undefined;
const inViewWatchers = new Set<() => void>();
export function showInView(n: number | undefined) {
  pageInView = n;
  for (const w of inViewWatchers) w();
}
const watchInView = (w: () => void) => {
  inViewWatchers.add(w);
  return () => inViewWatchers.delete(w);
};

/**
 * "Page N" before the count in the filter row's readout (results.tsx), from page 2 on: the server's page in its
 * HTML, then the page in view as the reader scrolls, so it moves with the address.
 */
export function PageInView({ page }: { page: number }) {
  const { t } = useUi();
  const n = useSyncExternalStore(
    watchInView,
    () => pageInView ?? page,
    () => page,
  );
  if (n < 2) return null;
  // one item, so the readout can set it apart from the count (app/filter-row.css, .page-cue)
  return (
    <HStack gap={2} vAlign="center" className="page-cue">
      <Text type="supporting" color="secondary" hasTabularNumbers>
        {t("list.page", { n })}
      </Text>
      <Text type="supporting" color="secondary">
        ·
      </Text>
    </HStack>
  );
}
