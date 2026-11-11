"use client";
// Settings, template settings-dialog (page.tsx:2271-2603) as is: a 1120 px Dialog of fixed height; on a laptop
// a SideNav rail (title, sections of panels) beside the scrolling panel with its sticky close button, below
// 640 px a title row, a tab strip of the panels and the panel under it. Filled with BoothDex's panels
// (panels.tsx; Devices only when signed in). The template's settings search is left out: four panels of six rows
// are read at a glance. Nothing in the panels is typed, so a click on the backdrop closes it like Escape does;
// the account deletion form opens its own dialog, which keeps that click from losing a half-typed password.

import { Button } from "@astryxdesign/core/Button";
import { Dialog } from "@astryxdesign/core/Dialog";
import { Divider } from "@astryxdesign/core/Divider";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import {
  SideNav,
  SideNavItem,
  SideNavSection,
} from "@astryxdesign/core/SideNav";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import { type Ref, useId, useState } from "react";
import { useUi } from "@/components/providers";
import { DIALOG_MAX_HEIGHT } from "@/lib/dialog-height";
import { useScrollTop } from "@/lib/use-scroll-top";
import {
  PANEL_GROUPS,
  type PanelId,
  SettingsPanelBody,
  usePanelGroups,
} from "./panels";
import "@/lib/icons-on-demand";

const NARROW_VIEWPORT = "(max-width: 640px)";
const PANELS = PANEL_GROUPS.flatMap((group) => group.panels);
const panelConfig = (id: PanelId) =>
  PANELS.find((panel) => panel.id === id) ?? PANELS[0];

function SettingsPanelHeading({ panel }: { panel: PanelId }) {
  const { t } = useUi();
  const config = panelConfig(panel);
  return (
    <VStack gap={0.5}>
      <Heading level={2}>{t(config.label)}</Heading>
      <Text type="supporting" color="secondary">
        {t(config.description)}
      </Text>
    </VStack>
  );
}

function CloseButton({ onClose }: { onClose: () => void }) {
  const { t } = useUi();
  return (
    <IconButton
      label={t("common.close")}
      variant="ghost"
      size="sm"
      icon={<Icon icon="close" size="sm" />}
      onClick={onClose}
    />
  );
}

function SettingsSideNav({
  titleId,
  activePanel,
  onSelectPanel,
}: {
  titleId: string;
  activePanel: PanelId;
  onSelectPanel: (panel: PanelId) => void;
}) {
  const { t } = useUi();
  const groups = usePanelGroups();
  return (
    <SideNav
      aria-label={t("settings.sections")}
      style={{ flexShrink: 0 }}
      header={
        <HStack gap={2} vAlign="center" paddingInline={2} minHeight={32}>
          <Heading level={4} id={titleId}>
            {t("settings.title")}
          </Heading>
        </HStack>
      }
    >
      {groups.map((group) => (
        <SideNavSection
          key={group.label}
          title={t(group.label)}
          style={{ paddingBlock: "var(--spacing-2)" }}
        >
          {group.panels.map((panel) => (
            <SideNavItem
              key={panel.id}
              label={t(panel.label)}
              icon={<Icon icon={panel.icon} size="sm" color="primary" />}
              isSelected={panel.id === activePanel}
              onClick={() => onSelectPanel(panel.id)}
            />
          ))}
        </SideNavSection>
      ))}
    </SideNav>
  );
}

function SettingsContentPane({
  panel,
  scroller,
  onClose,
}: {
  panel: PanelId;
  scroller: Ref<HTMLDivElement>;
  onClose: () => void;
}) {
  return (
    <LayoutContent ref={scroller} isScrollable padding={4}>
      {/* the close button stays at the pane's top-right corner while the panel scrolls (page.tsx:2366-2380) */}
      <HStack
        width="100%"
        hAlign="end"
        style={{ position: "sticky", insetBlockStart: 0, zIndex: 1, height: 0 }}
      >
        <HStack
          style={{
            display: "flex",
            backgroundColor: "var(--color-background-surface)",
            borderRadius: "var(--radius-element)",
          }}
        >
          <CloseButton onClose={onClose} />
        </HStack>
      </HStack>
      <VStack gap={4} className="settings-panel">
        <HStack gap={2} vAlign="start">
          <StackItem size="fill">
            <SettingsPanelHeading panel={panel} />
          </StackItem>
          {/* the column the pinned close button floats over */}
          <VStack aria-hidden="true" width="var(--size-element-md)" />
        </HStack>
        <SettingsPanelBody panel={panel} />
      </VStack>
    </LayoutContent>
  );
}

function SettingsNarrowShell({
  titleId,
  activePanel,
  onSelectPanel,
  scroller,
  onClose,
}: {
  titleId: string;
  activePanel: PanelId;
  onSelectPanel: (panel: PanelId) => void;
  scroller: Ref<HTMLDivElement>;
  onClose: () => void;
}) {
  const { t } = useUi();
  const panels = usePanelGroups().flatMap((group) => group.panels);
  return (
    <VStack gap={0} height="100%">
      <HStack gap={2} vAlign="center" paddingInline={3} paddingBlock={3}>
        <StackItem size="fill">
          <Text type="label" id={titleId}>
            {t("settings.title")}
          </Text>
        </StackItem>
        <CloseButton onClose={onClose} />
      </HStack>
      <Divider />
      <StackItem size="static">
        <HStack
          as="nav"
          aria-label={t("settings.sections")}
          gap={1}
          wrap="nowrap"
          isScrollable
          paddingInline={3}
          paddingBlock={2}
        >
          {panels.map((panel) => (
            <Button
              key={panel.id}
              label={t(panel.label)}
              icon={<Icon icon={panel.icon} size="sm" />}
              variant={panel.id === activePanel ? "secondary" : "ghost"}
              size="sm"
              // the strip scrolls; its tabs keep their whole names
              style={{ flexShrink: 0 }}
              onClick={() => onSelectPanel(panel.id)}
            />
          ))}
        </HStack>
      </StackItem>
      <Divider />
      <StackItem size="fill">
        <VStack
          ref={scroller}
          gap={4}
          padding={4}
          isScrollable
          height="100%"
          className="settings-panel"
        >
          <SettingsPanelHeading panel={activePanel} />
          <SettingsPanelBody panel={activePanel} />
        </VStack>
      </StackItem>
    </VStack>
  );
}

export default function SettingsDialog({
  isOpen,
  onOpenChange,
}: {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [chosen, setActivePanel] = useState<PanelId>("appearance");
  // a panel that went away (Devices after signing out) falls back to the first
  const shown = usePanelGroups().flatMap((group) => group.panels);
  const activePanel = shown.some((p) => p.id === chosen) ? chosen : shown[0].id;
  const titleId = useId();
  const isNarrow = useMediaQuery(NARROW_VIEWPORT);
  const close = () => onOpenChange(false);
  // a panel, picked or opened again, starts at its top
  const scroller = useScrollTop(isOpen && activePanel);
  return (
    <Dialog
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      width="70rem"
      maxHeight={DIALOG_MAX_HEIGHT}
      padding={0}
      aria-labelledby={titleId}
    >
      <VStack height={DIALOG_MAX_HEIGHT}>
        {isNarrow ? (
          <SettingsNarrowShell
            titleId={titleId}
            activePanel={activePanel}
            onSelectPanel={setActivePanel}
            scroller={scroller}
            onClose={close}
          />
        ) : (
          <Layout
            start={
              <SettingsSideNav
                titleId={titleId}
                activePanel={activePanel}
                onSelectPanel={setActivePanel}
              />
            }
            content={
              <SettingsContentPane
                panel={activePanel}
                scroller={scroller}
                onClose={close}
              />
            }
          />
        )}
      </VStack>
    </Dialog>
  );
}
