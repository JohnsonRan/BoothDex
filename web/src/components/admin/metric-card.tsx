// A number tile: the Astryx dashboard template's MetricCard (templates/pages/dashboard/page.tsx:586-623), with
// a warning dot beside the value while the number asks for attention, an optional progress bar for a share
// (the template's ProgressBar, :369-375) and an optional action at the bottom.

import { Card } from "@astryxdesign/core/Card";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { ProgressBar } from "@astryxdesign/core/ProgressBar";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { Heading, Text } from "@astryxdesign/core/Text";

export function MetricCard({
  label,
  value,
  sub,
  isWarning,
  percent,
  action,
}: {
  label: string;
  value: string;
  sub?: React.ReactNode;
  isWarning?: boolean;
  /** 0-100; above 90 the bar turns to the error colour */
  percent?: number;
  action?: React.ReactNode;
}) {
  return (
    <Card>
      <VStack gap={2}>
        <Heading level={4} accessibilityLevel={3}>
          {label}
        </Heading>
        <HStack gap={2} vAlign="center">
          <Heading level={2} accessibilityLevel={4}>
            {value}
          </Heading>
          {isWarning && <StatusDot variant="warning" label={label} />}
        </HStack>
        {percent !== undefined && (
          <ProgressBar
            label={label}
            isLabelHidden
            value={percent}
            variant={percent > 90 ? "error" : "accent"}
          />
        )}
        {sub && (
          <Text type="supporting" color="secondary">
            {sub}
          </Text>
        )}
        {action && <HStack>{action}</HStack>}
      </VStack>
    </Card>
  );
}
