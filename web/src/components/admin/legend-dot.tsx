// A chart series' swatch and name, as the Astryx dashboard templates draw their legends
// (templates/pages/dashboard-scorecard/page.tsx:383-395): a small square in the series colour, the name in
// supporting text. The colour is inline because Icon's `color` takes only semantic names.

import { Icon } from "@astryxdesign/core/Icon";
import { HStack } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";

export function LegendDot({
  color,
  label,
  value,
}: {
  color: string;
  label: string;
  value?: string;
}) {
  return (
    <HStack gap={2} vAlign="center" wrap="nowrap">
      <Icon icon="admin:swatch" size="xsm" style={{ color }} />
      <Text type="supporting" color="secondary">
        {label}
      </Text>
      {value !== undefined && (
        <Text type="supporting" weight="semibold" hasTabularNumbers>
          {value}
        </Text>
      )}
    </HStack>
  );
}
