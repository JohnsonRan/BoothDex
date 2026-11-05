"use client";
// The settings dialog's card of rows, copied from template settings-dialog (page.tsx:587-688, SettingsCard and
// SettingsRow): a muted Card of divided rows, each with an icon, a label, an optional description and a control
// in one column, every row of a panel stacking at the same width. The template's StyleX rules for that (row.*,
// page.tsx:259-292) are the `settings-*` classes in globals.css: this app does not compile StyleX.

import { Card } from "@astryxdesign/core/Card";
import { Divider } from "@astryxdesign/core/Divider";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, Stack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { Children, type ReactNode } from "react";

/** The one width every selector in a row takes, so the controls form one column (page.tsx:207). */
export const CONTROL_WIDTH = 192;

/** The one inset for this surface: spacing step 4 === 16px (page.tsx:193). */
const ROW_PADDING = 4;

export function SettingsCard({
  title,
  children,
}: {
  title?: string;
  children: ReactNode;
}) {
  const rows = Children.toArray(children);
  return (
    <VStack gap={1.5}>
      {title != null && (
        <Text type="supporting" weight="semibold" color="secondary">
          {title}
        </Text>
      )}
      <Card padding={0} width="100%" variant="muted">
        <VStack as="ul" role="list" gap={0}>
          {rows.map((row, index) => (
            // rows are fixed per panel and never reorder
            // biome-ignore lint/suspicious/noArrayIndexKey: as the template
            <VStack key={index} as="li" gap={0}>
              {index > 0 && <Divider variant="subtle" />}
              {row}
            </VStack>
          ))}
        </VStack>
      </Card>
    </VStack>
  );
}

export function SettingsRow({
  title,
  description,
  icon,
  control,
  detail,
}: {
  title: string;
  description?: ReactNode;
  icon: `site:${string}` | "info";
  control?: ReactNode;
  detail?: ReactNode;
}) {
  return (
    <VStack padding={ROW_PADDING} gap={2}>
      <Stack className="settings-line">
        <StackItem size="fill">
          <HStack gap={2} vAlign="start">
            {/* 2px down: a glyph is optically centred in its box, a cap-height sits low (page.tsx:665-668) */}
            <VStack paddingBlockStart={0.5}>
              <Icon icon={icon} size="sm" color="secondary" />
            </VStack>
            <VStack gap={0.5} className="settings-text">
              <Text type="label">{title}</Text>
              {description != null && (
                <Text type="supporting" color="secondary">
                  {description}
                </Text>
              )}
            </VStack>
          </HStack>
        </StackItem>
        {control != null && (
          <VStack className="settings-control">{control}</VStack>
        )}
      </Stack>
      {detail}
    </VStack>
  );
}
