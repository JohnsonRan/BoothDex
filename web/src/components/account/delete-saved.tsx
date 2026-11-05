"use client";
// Deleting a saved search asks first, in a Popover under its button (block PopoverConfirmAction: heading,
// destructive Delete, ghost Cancel).

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Popover } from "@astryxdesign/core/Popover";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Heading } from "@astryxdesign/core/Text";
import { useState } from "react";
import { useUi } from "@/components/providers";

export function DeleteSaved({
  name,
  onConfirm,
}: {
  name: string;
  onConfirm: () => void;
}) {
  const { t } = useUi();
  const [open, setOpen] = useState(false);
  const label = t("saved.delete", { name });
  return (
    <Popover
      placement="below"
      alignment="end"
      label={label}
      width={300}
      isOpen={open}
      onOpenChange={setOpen}
      content={
        <VStack gap={3}>
          <Heading level={4}>{t("saved.confirmDelete", { name })}</Heading>
          <HStack gap={2} hAlign="end">
            <Button
              label={t("common.delete")}
              variant="destructive"
              onClick={() => {
                setOpen(false);
                onConfirm();
              }}
            />
            <Button
              label={t("common.cancel")}
              variant="ghost"
              onClick={() => setOpen(false)}
            />
          </HStack>
        </VStack>
      }
    >
      <IconButton
        label={label}
        tooltip={label}
        variant="ghost"
        size="sm"
        icon={<Icon icon="site:clear" size="sm" />}
      />
    </Popover>
  );
}
