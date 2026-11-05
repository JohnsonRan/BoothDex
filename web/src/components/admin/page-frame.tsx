"use client";
// The frame of every admin page, from the header of the Astryx page template detail-page
// (templates/pages/detail-page/page.tsx:169-262): a LayoutHeader with a divider and the template's padding,
// the h1 with a short status beside it, and on pages with sub-pages the large full-bleed TabList as the
// header's last row, sitting on the divider (no bottom padding). The content keeps the same padding, so it
// starts on the header's left edge.

import {
  HStack,
  Layout,
  LayoutContent,
  LayoutHeader,
  VStack,
} from "@astryxdesign/core/Layout";
import { Heading } from "@astryxdesign/core/Text";

export function PageFrame({
  title,
  meta,
  tabs,
  children,
}: {
  title: string;
  meta?: React.ReactNode;
  /** a `TabList size="lg" isFullBleed` */
  tabs?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <Layout
      height="fill"
      contentWidth={1440}
      header={
        <LayoutHeader
          hasDivider
          padding={4}
          paddingBlockEnd={tabs ? 0 : undefined}
        >
          <VStack gap={3}>
            <HStack gap={3} vAlign="center" wrap="wrap">
              <Heading level={1}>{title}</Heading>
              {meta}
            </HStack>
            {tabs}
          </VStack>
        </LayoutHeader>
      }
      content={
        <LayoutContent padding={4}>
          <VStack gap={6}>{children}</VStack>
        </LayoutContent>
      }
    />
  );
}
