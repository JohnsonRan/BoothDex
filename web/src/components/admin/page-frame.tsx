"use client";
// The frame of every admin page, from the header of the Astryx page template detail-page
// (templates/pages/detail-page/page.tsx:169-262): a LayoutHeader with a divider and the template's padding,
// the h1 with a short status beside it, and on pages with sub-pages the large full-bleed TabList as the
// header's last row, sitting on the divider (no bottom padding). The content keeps the same padding, so it
// starts on the header's left edge. Until the admin probe succeeds, the console's gate (checking, sign in, not
// an admin, unreachable) stands in for the content, and a Skeleton one supporting line high for the title's
// status: the page keeps its own title and tabs from the first paint, so nothing moves when the data arrives.
// On a phone the status always takes its own line under the title, so whether it fits beside the title can
// not change the header's height.

import { useAppShellMobile } from "@astryxdesign/core/AppShell";
import {
  HStack,
  Layout,
  LayoutContent,
  LayoutHeader,
  VStack,
} from "@astryxdesign/core/Layout";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { Heading } from "@astryxdesign/core/Text";
import { useGate } from "./admin-console";

/** one line of supporting text, the height of the title's status */
const STATUS_LINE =
  "calc(var(--text-supporting-size) * var(--text-supporting-leading))";

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
  const gate = useGate();
  const isMobile = useAppShellMobile().isMobile;
  const heading = <Heading level={1}>{title}</Heading>;
  const status =
    gate && meta ? <Skeleton width="12rem" height={STATUS_LINE} /> : meta;
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
            {isMobile ? (
              <>
                {heading}
                {status}
              </>
            ) : (
              <HStack gap={3} vAlign="center" wrap="wrap">
                {heading}
                {status}
              </HStack>
            )}
            {tabs}
          </VStack>
        </LayoutHeader>
      }
      content={
        <LayoutContent padding={4}>
          <VStack gap={6}>{gate ?? children}</VStack>
        </LayoutContent>
      }
    />
  );
}
