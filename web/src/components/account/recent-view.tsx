"use client";
// The recent view's parts as the home page places them, loaded only on that view: the list is read in the
// browser anyway, and its code then stays out of every other page's bundle. The silhouettes hold the places
// while it loads, as they do for the server's results.

import dynamic from "next/dynamic";
import {
  CountSilhouette,
  ResultsSilhouette,
} from "@/components/catalog/loading";
import { useUi } from "@/components/providers";

function GridSilhouette() {
  const { t } = useUi();
  return <ResultsSilhouette label={t("list.loading")} />;
}

export const RecentCount = dynamic(
  () => import("./recent").then((m) => m.RecentCount),
  { ssr: false, loading: CountSilhouette },
);
export const RecentResults = dynamic(
  () => import("./recent").then((m) => m.RecentResults),
  { ssr: false, loading: GridSilhouette },
);
