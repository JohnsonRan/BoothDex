"use client";
// The recharts plot behind trend-chart.tsx: a 2 px line per series over the same days with a wash of the series
// colour under it that fades out downward, so series overlap without stacking or turning grey. The grid and
// ticks follow the Astryx template dashboard-scorecard (templates/pages/dashboard-scorecard/page.tsx:101-115,
// :611-626): horizontal lines only, tick labels in the secondary text colour, no axis lines; five value ticks
// on a desktop, three on a phone. Hovering a day, or focusing the plot and pressing the arrow keys (recharts'
// keyboard layer), draws a guide line and one tooltip with every series' value for that day; a pointer press
// does not focus the plot, so the browser's square outline appears only for keyboard focus.

import { Card } from "@astryxdesign/core/Card";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { useId } from "react";
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { useUi } from "@/components/providers";
import { LOCALES } from "@/lib/settings-core";
import { LegendDot } from "./legend-dot";
import type { TrendSeries } from "./trend-chart";

const GRID = "var(--color-border)";
const TICK = {
  fontSize: "var(--font-size-sm)",
  fill: "var(--color-text-secondary)",
};
/** the wash's opacity at the line; it fades to 0 at the baseline */
const WASH_TOP = 0.16;
const TICK_COUNT = { wide: 5, phone: 3 } as const;
/** room for half of the last day label, which centres on the plot's right edge */
const MARGIN = { top: 4, right: 20, left: 0, bottom: 0 };
/** on a phone the tooltip starts at the plot's left edge, so a long readout never runs off screen */
const PHONE_TOOLTIP = { x: 0 };

type Row = { day: string } & Record<string, number | string>;

export default function TrendPlot({
  days,
  series,
  caption,
  height,
  phone,
}: {
  days: string[];
  series: TrendSeries[];
  caption: string;
  height: string;
  phone: boolean;
}) {
  const locale = LOCALES[useUi().ui];
  const wash = useId().replace(/:/g, "");
  // days are UTC dates; read them in UTC so no time zone moves a point to its neighbour
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
  return (
    <VStack
      role="group"
      aria-label={caption}
      height={height}
      onMouseDownCapture={(e) => e.preventDefault()}
    >
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={rows} margin={MARGIN}>
          <defs>
            {series.map((s) => (
              <linearGradient
                key={s.key}
                id={`${wash}-${s.key}`}
                x1="0"
                y1="0"
                x2="0"
                y2="1"
              >
                <stop offset="0%" stopColor={s.color} stopOpacity={WASH_TOP} />
                <stop offset="100%" stopColor={s.color} stopOpacity={0} />
              </linearGradient>
            ))}
          </defs>
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
            tickCount={TICK_COUNT[phone ? "phone" : "wide"]}
            allowDecimals={false}
            tickFormatter={(v: number) =>
              v.toLocaleString(locale, { notation: "compact" })
            }
            tick={TICK}
            axisLine={false}
            tickLine={false}
          />
          <Tooltip
            position={phone ? PHONE_TOOLTIP : undefined}
            isAnimationActive={false}
            cursor={{ stroke: GRID }}
            wrapperStyle={{ pointerEvents: "none", zIndex: 1 }}
            content={({ active, payload }) => {
              const row = payload?.[0]?.payload as Row | undefined;
              if (!active || !row) return null;
              return (
                <Card padding={3} elevation="med">
                  <VStack gap={1}>
                    <Text type="supporting" color="secondary">
                      {date(row.day, "long")}
                    </Text>
                    {series.map((s) => (
                      <HStack
                        key={s.key}
                        gap={4}
                        hAlign="between"
                        vAlign="center"
                        wrap="nowrap"
                      >
                        <LegendDot color={s.color} label={s.label} />
                        <Text weight="semibold" hasTabularNumbers>
                          {Number(row[s.key]).toLocaleString(locale)}
                        </Text>
                      </HStack>
                    ))}
                  </VStack>
                </Card>
              );
            }}
          />
          {series.map((s) => (
            <Area
              key={s.key}
              type="monotone"
              dataKey={s.key}
              name={s.label}
              stroke={s.color}
              strokeWidth={2}
              fill={`url(#${wash}-${s.key})`}
              dot={false}
              isAnimationActive={false}
            />
          ))}
        </AreaChart>
      </ResponsiveContainer>
    </VStack>
  );
}
