"use client";
// The settings panels' form dialog: the Astryx block Dialog — Form (DialogFormDialog: DialogHeader with an optional
// subtitle, the fields in LayoutContent, Cancel and the action in LayoutFooter). The fields sit in a <form>, so
// Enter submits and password managers fill them; the footer's button submits it through its `form` id.
// contentWidth="none": opened inside the settings dialog, the Layout would take that dialog's
// --layout-content-width and indent its header 16 px past the fields on a phone.

import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import {
  HStack,
  Layout,
  LayoutContent,
  LayoutFooter,
} from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/Stack";
import { type ReactNode, useId } from "react";
import { useUi } from "@/components/providers";

export function FormDialog({
  title,
  subtitle,
  action,
  isDestructive = false,
  isBusy,
  onClose,
  onSubmit,
  children,
}: {
  title: string;
  subtitle?: string;
  /** the submit button's label: the action it takes */
  action: string;
  isDestructive?: boolean;
  isBusy: boolean;
  onClose: () => void;
  onSubmit: () => void;
  children: ReactNode;
}) {
  const { t } = useUi();
  const form = useId();
  return (
    <Dialog isOpen onOpenChange={onClose} purpose="form" width={480}>
      <Layout
        contentWidth="none"
        header={
          <DialogHeader
            title={title}
            subtitle={subtitle}
            onOpenChange={onClose}
          />
        }
        content={
          <LayoutContent>
            <form
              id={form}
              noValidate
              onSubmit={(e) => {
                e.preventDefault();
                onSubmit();
              }}
            >
              <VStack gap={2}>{children}</VStack>
            </form>
          </LayoutContent>
        }
        footer={
          <LayoutFooter>
            <HStack gap={2} hAlign="end">
              <Button
                label={t("common.cancel")}
                variant="secondary"
                onClick={onClose}
              />
              <Button
                type="submit"
                form={form}
                label={action}
                variant={isDestructive ? "destructive" : "primary"}
                isLoading={isBusy}
              />
            </HStack>
          </LayoutFooter>
        }
      />
    </Dialog>
  );
}
