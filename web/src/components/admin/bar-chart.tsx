"use client";
// A daily bar chart in a card, from the Astryx template dashboard-scorecard's TrendChart
// (templates/pages/dashboard-scorecard/page.tsx:522-660): title with the headline figure (:535-541), the plot
// at least 200 px (:548), the legend as text under it (:653-656). recharts loads on first use (bar-plot.tsx), so it
// stays out of every other admin page; until then, and while the data is pending, a Skeleton of the plot's size
// holds the space.

import { Card } from "@astryxdesign/core/Card";
import { Center } from "@astryxdesign/core/Center";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Layout";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { Heading, Text } from "@astryxdesign/core/Text";
import { lazy, Suspense, useEffect } from "react";
import { LegendDot } from "./legend-dot";

export type BarSeries = {
  key: string;
  label: string;
  /** an Astryx data token, e.g. `var(--color-data-categorical-blue)` */
  color: string;
  values: number[];
};

/** the template's 200 px plot (:548), in rem so it follows the root font size */
const PLOT_HEIGHT = "12.5rem";

// Once loaded, the plot renders directly: a lazy component would suspend once more on every new mount.
let loaded: typeof import("./bar-plot") | null = null;
const load = () => import("./bar-plot").then((m) => (loaded = m));
const LazyPlot = lazy(load);

export function BarChart({
  title,
  headline,
  action,
  days,
  series,
  empty,
}: {
  title: string;
  headline?: string;
  action?: React.ReactNode;
  /** UTC days, `YYYY-MM-DD`; undefined while loading */
  days?: string[];
  series: BarSeries[];
  /** shown in place of the plot when every value is 0 */
  empty: string;
}) {
  useEffect(() => void load(), []);
  const skeleton = <Skeleton width="100%" height="100%" />;
  const blank = days && series.every((s) => s.values.every((v) => v === 0));
  const plot = { days: days ?? [], series, caption: title };
  return (
    <Card padding={5} height="100%">
      <VStack gap={6} height="100%">
        <HStack gap={2} hAlign="between" vAlign="center" wrap="wrap">
          <HStack gap={2} wrap="wrap">
            <Heading level={3}>{title}</Heading>
            {headline && (
              <Heading level={3} color="secondary">
                {headline}
              </Heading>
            )}
          </HStack>
          {action}
        </HStack>
        {/* the plot takes the height its row leaves, so charts side by side end level */}
        <StackItem size="fill">
          <VStack height="100%" minHeight={PLOT_HEIGHT}>
            {!days ? (
              skeleton
            ) : blank ? (
              <Center height="100%">
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
        </StackItem>
        {series.length > 1 && (
          <HStack gap={5} vAlign="center" wrap="wrap">
            {series.map((s) => (
              <LegendDot key={s.key} color={s.color} label={s.label} />
            ))}
          </HStack>
        )}
      </VStack>
    </Card>
  );
}
