"use client";
// The frame of every admin page, from the header of the Astryx page templates incident-console
// (templates/pages/incident-console/page.tsx:492-523) and settings (page.tsx:73-91): a LayoutHeader with a
// divider holding the h1, a short status beside it, then the content with the settings
// template's padding (page.tsx:94), which puts it on the header's left edge. An optional tab row sits under
// the title, as the reference admin puts sub-pages.

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
  tabs?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <Layout
      height="fill"
      contentWidth={1440}
      header={
        <LayoutHeader hasDivider>
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
