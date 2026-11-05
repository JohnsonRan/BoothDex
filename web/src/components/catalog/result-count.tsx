"use client";

import { Spinner } from "@astryxdesign/core/Spinner";
import { Text } from "@astryxdesign/core/Text";
import { useUi } from "@/components/providers";
import { LOCALES } from "@/lib/settings-core";
import { useFilterUrl } from "./filter-url";

/** Result count beside the title; while new results load it turns into a spinner. */
export function ResultCount({ count }: { count: number }) {
  const { t, ui } = useUi();
  const { pending } = useFilterUrl();
  return pending ? (
    <Spinner size="sm" aria-label={t("list.loading")} />
  ) : (
    <Text type="supporting" hasTabularNumbers>
      {t("title.count", { n: count.toLocaleString(LOCALES[ui]) })}
    </Text>
  );
}
