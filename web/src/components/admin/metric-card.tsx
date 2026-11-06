// A number tile: the Astryx template dashboard-scorecard's ScorecardTile
// (templates/pages/dashboard-scorecard/page.tsx:461-477: label in label text, the value as a level-2 heading)
// with the label's icon, as the reference admin tags its tiles. A status dot beside the value only while the
// number asks for attention, an optional usage bar (the template dashboard's ProgressBar,
// templates/pages/dashboard/page.tsx:369-375) and one supporting line. The card fills its grid cell, so the
// tiles of a row share one height.

import { Card } from "@astryxdesign/core/Card";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { ProgressBar } from "@astryxdesign/core/ProgressBar";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { Heading, Text } from "@astryxdesign/core/Text";
import { useAdminT } from "./messages";

/** Usage at or above these percentages turns the bar to warning, then error. */
const USAGE_WARN = 80;
const USAGE_HIGH = 90;

export function MetricCard({
  label,
  icon,
  value,
  sub,
  attention,
  usage,
}: {
  label: string;
  icon: `admin:${string}`;
  value: string;
  sub?: React.ReactNode;
  attention?: "warning" | "error";
  /** 0-100, a share of a resource in use */
  usage?: number;
}) {
  const t = useAdminT();
  const note = attention && t(`tile.${attention}`);
  return (
    <Card padding={4} height="100%">
      <VStack gap={1}>
        <HStack gap={2} vAlign="center" wrap="nowrap">
          <Icon icon={icon} size="sm" color="secondary" />
          <Text type="label" color="secondary">
            {label}
          </Text>
        </HStack>
        <HStack gap={2} vAlign="center">
          <Heading level={2} accessibilityLevel={3}>
            {value}
          </Heading>
          {note && (
            <StatusDot variant={attention} label={note} tooltip={note} />
          )}
        </HStack>
        {usage !== undefined && (
          <ProgressBar
            label={label}
            isLabelHidden
            value={usage}
            variant={
              usage >= USAGE_HIGH
                ? "error"
                : usage >= USAGE_WARN
                  ? "warning"
                  : "accent"
            }
          />
        )}
        {sub && (
          <Text type="supporting" color="secondary">
            {sub}
          </Text>
        )}
      </VStack>
    </Card>
  );
}
