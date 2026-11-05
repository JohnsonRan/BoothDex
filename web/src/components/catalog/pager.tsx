"use client";

import { Center } from "@astryxdesign/core/Center";
import { Pagination } from "@astryxdesign/core/Pagination";
import { useUi } from "@/components/providers";
import { useFilterUrl } from "./filter-url";

export function Pager({
  page,
  total,
  noSemantic,
}: {
  page: number;
  total: number;
  noSemantic: boolean;
}) {
  const { t } = useUi();
  // not disabled while a page loads: the refresh progress says so, and the pager keeps its layout
  const { goPage } = useFilterUrl();
  if (total <= 1) return null;
  return (
    <Center>
      <Pagination
        label={t("page.nav")}
        page={page}
        totalPages={total}
        onChange={(n) => goPage(n, noSemantic)}
      />
    </Center>
  );
}
