// A titled band of the overview, from the Astryx template dashboard-scorecard: the section heading
// (templates/pages/dashboard-scorecard/page.tsx:724-735) with the section's action at its end, as the template
// dashboard puts "View more" (templates/pages/dashboard/page.tsx:766-768), then the body 24 px below
// (scorecard :818). Tiles sit in the scorecard's tight grid (:820, gap 8 px): up to four across on a desktop,
// two on a phone, and a band with fewer tiles than columns shares the row out; the band's charts follow at the
// same gap, so tiles and charts read as one block.

import { Grid } from "@astryxdesign/core/Grid";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { Heading } from "@astryxdesign/core/Text";

const TILES = { minWidth: 160, max: 4, repeat: "fit" } as const;
/** Charts side by side once each can be 320 px wide. */
const CHARTS = { minWidth: 320, repeat: "fit" } as const;

export function Band({
  title,
  action,
  children,
}: {
  title: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <VStack gap={6}>
      <HStack gap={3} hAlign="between" vAlign="center">
        <Heading level={2}>{title}</Heading>
        {action}
      </HStack>
      <VStack gap={2}>{children}</VStack>
    </VStack>
  );
}

export function Tiles({ children }: { children: React.ReactNode }) {
  return (
    <Grid columns={TILES} gap={2}>
      {children}
    </Grid>
  );
}

export function Charts({ children }: { children: React.ReactNode }) {
  return (
    <Grid columns={CHARTS} gap={2}>
      {children}
    </Grid>
  );
}
