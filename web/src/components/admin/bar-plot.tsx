"use client";
// The recharts plot behind bar-chart.tsx, with the template's axes and grid
// (templates/pages/dashboard-scorecard/page.tsx:101-115, :611-626): recessive grid lines, tick labels in the
// secondary text colour, no axis lines. Several series stack, one bar per day, with a 2 px gap of the card's
// colour between segments; a lone series gets the rounded data end. Hovering a day, or focusing the plot and
// pressing the arrow keys (recharts' keyboard layer), shows every series' value for that day.

import { Card } from "@astryxdesign/core/Card";
import { VStack } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { useUi } from "@/components/providers";
import { LOCALES } from "@/lib/settings-core";
import type { BarSeries } from "./bar-chart";
import { LegendDot } from "./legend-dot";

const GRID = "var(--color-border)";
const TICK = {
  fontSize: "var(--font-size-sm)",
  fill: "var(--color-text-secondary)",
};
const GAP = { stroke: "var(--color-background-card)", strokeWidth: 2 };
const MARGIN = { top: 5, right: 8, left: 0, bottom: 0 };

type Row = { day: string } & Record<string, number | string>;

export default function BarPlot({
  days,
  series,
  caption,
}: {
  days: string[];
  series: BarSeries[];
  caption: string;
}) {
  const locale = LOCALES[useUi().ui];
  // days are UTC dates; read them in UTC so no time zone moves a bar to its neighbour
  const date = (day: string, month: "numeric" | "long") =>
    new Date(`${day}T00:00:00Z`).toLocaleDateString(locale, {
      timeZone: "UTC",
      month,
      day: "numeric",
    });
  const rows: Row[] = days.map((day, i) => ({
    day,
    ...Object.fromEntries(series.map((s) => [s.key, s.values[i]])),
  }));
  const stacked = series.length > 1;
  return (
    <VStack role="group" aria-label={caption} height="100%">
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={rows} margin={MARGIN}>
          <CartesianGrid horizontal vertical={false} stroke={GRID} />
          <XAxis
            dataKey="day"
            tickFormatter={(d: string) => date(d, "numeric")}
            interval="equidistantPreserveStart"
            minTickGap={16}
            tick={TICK}
            axisLine={false}
            tickLine={false}
          />
          <YAxis
            width="auto"
            allowDecimals={false}
            tickFormatter={(v: number) =>
              v.toLocaleString(locale, { notation: "compact" })
            }
            tick={TICK}
            axisLine={false}
            tickLine={false}
          />
          <Tooltip
            isAnimationActive={false}
            cursor={{ fill: "var(--color-background-muted)" }}
            wrapperStyle={{ pointerEvents: "none", zIndex: 1 }}
            content={({ active, payload }) => {
              const row = payload?.[0]?.payload as Row | undefined;
              if (!active || !row) return null;
              return (
                <Card padding={3}>
                  <VStack gap={1}>
                    <Text type="supporting" color="secondary">
                      {date(row.day, "long")}
                    </Text>
                    {series.map((s) => (
                      <LegendDot
                        key={s.key}
                        color={s.color}
                        label={s.label}
                        value={Number(row[s.key]).toLocaleString(locale)}
                      />
                    ))}
                  </VStack>
                </Card>
              );
            }}
          />
          {series.map((s) => (
            <Bar
              key={s.key}
              dataKey={s.key}
              name={s.label}
              fill={s.color}
              stackId={stacked ? "day" : undefined}
              radius={stacked ? 0 : [4, 4, 0, 0]}
              {...(stacked ? GAP : {})}
              isAnimationActive={false}
            />
          ))}
        </BarChart>
      </ResponsiveContainer>
    </VStack>
  );
}
