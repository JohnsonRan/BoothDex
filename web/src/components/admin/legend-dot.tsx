// A chart series' key and name: a short 2 px bar in the series colour, the same weight as its line, beside the
// name in supporting text; Legend sets a chart's keys in one wrapping row. The colour is inline because no
// Astryx component takes an arbitrary data colour.

import { HStack, VStack } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";

const KEY = { width: "0.75rem", height: "0.125rem", flexShrink: 0 } as const;

export function LegendDot({ color, label }: { color: string; label: string }) {
  return (
    <HStack gap={2} vAlign="center" wrap="nowrap">
      <VStack aria-hidden style={{ ...KEY, backgroundColor: color }} />
      <Text type="supporting" color="secondary">
        {label}
      </Text>
    </HStack>
  );
}

export function Legend({
  items,
}: {
  items: { key: string; color: string; label: string }[];
}) {
  return (
    <HStack gap={4} vAlign="center" wrap="wrap">
      {items.map((s) => (
        <LegendDot key={s.key} color={s.color} label={s.label} />
      ))}
    </HStack>
  );
}
