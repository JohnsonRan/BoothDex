"use client";

// A daily trend in a card, the overview's one chart component, with the tiles' padding (16 px, metric-card.tsx)
// so the cards of a band share one inset. The card's title row (an h3 with an optional
// control at its end, as the Astryx template dashboard puts "View more" beside its card title,
// templates/pages/dashboard/page.tsx:766-768), then the legend when there are several series, then the plot:
// 14 rem high, 12 rem on a phone. recharts loads on first use (trend-plot.tsx), so it stays out of every other
// admin page; until then, and while the data is pending, a Skeleton of the plot's size sits under the real
// legend, so nothing moves when the plot arrives.

import { useAppShellMobile } from "@astryxdesign/core/AppShell";
import { Card } from "@astryxdesign/core/Card";
import { Center } from "@astryxdesign/core/Center";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { Heading, Text } from "@astryxdesign/core/Text";
import { lazy, Suspense, useEffect } from "react";
import { Legend } from "./legend-dot";

export type TrendSeries = {
  key: string;
  label: string;
  /** an Astryx data token, e.g. `var(--color-data-categorical-blue)` */
  color: string;
  values: number[];
};

const PLOT_HEIGHT = { wide: "14rem", phone: "12rem" } as const;

// Once loaded, the plot renders directly: a lazy component would suspend once more on every new mount.
let loaded: typeof import("./trend-plot") | null = null;
const load = () => import("./trend-plot").then((m) => (loaded = m));
const LazyPlot = lazy(load);

export function TrendChart({
  title,
  action,
  days,
  series,
  empty,
}: {
  title: string;
  action?: React.ReactNode;
  /** UTC days, `YYYY-MM-DD`; undefined while loading */
  days?: string[];
  series: TrendSeries[];
  /** shown in place of the plot when every value is 0 */
  empty: string;
}) {
  useEffect(() => void load(), []);
  const phone = useAppShellMobile().isMobile;
  const height = PLOT_HEIGHT[phone ? "phone" : "wide"];
  const skeleton = <Skeleton width="100%" height={height} />;
  const blank = days && series.every((s) => s.values.every((v) => v === 0));
  const plot = { days: days ?? [], series, caption: title, height, phone };
  return (
    <Card padding={4} height="100%">
      <VStack gap={4}>
        <HStack gap={3} hAlign="between" vAlign="center" wrap="wrap">
          <Heading level={3}>{title}</Heading>
          {action}
        </HStack>
        {series.length > 1 && <Legend items={series} />}
        {!days ? (
          skeleton
        ) : blank ? (
          <Center height={height}>
            <Text color="secondary">{empty}</Text>
          </Center>
        ) : loaded ? (
          <loaded.default {...plot} />
        ) : (
          <Suspense fallback={skeleton}>
            <LazyPlot {...plot} />
          </Suspense>
        )}
      </VStack>
    </Card>
  );
}
