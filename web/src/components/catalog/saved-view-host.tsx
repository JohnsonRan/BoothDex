"use client";
// The saved-view forms' host (table-filter template `AdaptiveSavedViewForm`, page.tsx:2122-2191).

import { BottomSheet } from "@astryxdesign/core/BottomSheet";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Section } from "@astryxdesign/core/Section";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import type { ReactNode } from "react";
import { useUi } from "@/components/providers";
import { SHEET_TOP_INSET } from "./filter-style";

/**
 * The template's form host (:2122-2191): a Dialog with a pointer, a tall sheet on a compact surface. Both are
 * `purpose="form"`, so a half-typed name survives a stray click or swipe; the sheet carries its own close.
 */
export function AdaptiveSavedViewForm({
  isOpen,
  onOpenChange,
  title,
  subtitle,
  isCompactSurface,
  children,
}: {
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  title: string;
  subtitle: string;
  isCompactSurface: boolean;
  children: ReactNode;
}) {
  const { t } = useUi();
  if (isCompactSurface) {
    return (
      <BottomSheet
        isOpen={isOpen}
        onOpenChange={onOpenChange}
        label={title}
        purpose="form"
        height="tall"
      >
        <Section
          variant="transparent"
          padding={4}
          paddingBlockStart={SHEET_TOP_INSET}
        >
          <VStack gap={4}>
            <HStack gap={2}>
              <StackItem size="fill">
                <VStack gap={1}>
                  <Heading level={3}>{title}</Heading>
                  <Text type="supporting" color="secondary">
                    {subtitle}
                  </Text>
                </VStack>
              </StackItem>
              <IconButton
                label={t("common.close")}
                variant="ghost"
                size="sm"
                icon={<Icon icon="close" size="sm" />}
                onClick={() => onOpenChange(false)}
              />
            </HStack>
            {children}
          </VStack>
        </Section>
      </BottomSheet>
    );
  }
  return (
    <Dialog
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      purpose="form"
      width={400}
    >
      <DialogHeader
        title={title}
        subtitle={subtitle}
        onOpenChange={onOpenChange}
        // dialogHeaderBleed (:1592-1596): one 16 px gutter for the header and the Section under it
        style={{
          marginInlineStart:
            "calc(-1 * var(--container-padding-inline-start, 0px))",
          marginInlineEnd:
            "calc(-1 * var(--container-padding-inline-end, 0px))",
          marginBlockStart:
            "calc(-1 * var(--container-padding-block-start, 0px))",
        }}
      />
      <Section variant="transparent" padding={4}>
        <VStack gap={4}>{children}</VStack>
      </Section>
    </Dialog>
  );
}
