"use client";
// The appearance cards, copied from template settings-dialog (page.tsx:1058-1218: ThemePreviewCanvas,
// ThemePreviewMock, ThemeChoiceCards): a SelectableCard per choice with a miniature page drawn by a nested Theme,
// then its icon and name. The colour mode's three are drawn in the neutral theme, each in its mode (System shows
// light and dark halves). The miniature's StyleX rules (page.tsx:294-322) are inline token styles here.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Grid } from "@astryxdesign/core/Grid";
import { Icon } from "@astryxdesign/core/Icon";
import { SelectableCard } from "@astryxdesign/core/SelectableCard";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import {
  Theme as AstryxTheme,
  type DefinedTheme,
} from "@astryxdesign/core/theme";
import type { CSSProperties } from "react";
import { defaultThemeSlug, themes } from "@/astryx-themes";
import { useUi } from "@/components/providers";
import type { MessageKey } from "@/lib/messages";
import type { Theme } from "@/lib/settings-core";

const bar: CSSProperties = {
  height: 4,
  width: "50%",
  borderRadius: 999,
  backgroundColor: "var(--color-border-emphasized)",
};

function ThemePreviewCanvas({
  theme,
  mode,
}: {
  theme: DefinedTheme;
  mode: "light" | "dark";
}) {
  return (
    <AstryxTheme theme={theme} mode={mode}>
      <VStack
        padding={2}
        gap={1.5}
        height="100%"
        style={{ backgroundColor: "var(--color-background-muted)" }}
      >
        <VStack gap={1} hAlign="center">
          <VStack style={{ ...bar, width: "66%" }} />
          <VStack style={bar} />
        </VStack>
        <StackItem size="fill">
          <VStack
            padding={2}
            gap={1}
            vAlign="center"
            height="100%"
            style={{ backgroundColor: "var(--color-background-surface)" }}
          >
            {[0, 1, 2].map((row) => (
              <VStack key={row} style={bar} />
            ))}
          </VStack>
        </StackItem>
      </VStack>
    </AstryxTheme>
  );
}

/** The theme in a mode. */
function ThemePreviewMock({ scheme }: { scheme: Theme }) {
  const theme = themes[defaultThemeSlug];
  return (
    <AspectRatio ratio={3 / 2} style={{ pointerEvents: "none" }}>
      <VStack
        aria-hidden="true"
        height="100%"
        style={{
          overflow: "hidden",
          borderRadius: "var(--radius-element)",
          backgroundColor: "var(--color-background-muted)",
        }}
      >
        {scheme === "system" ? (
          <HStack gap={0} height="100%">
            <StackItem size="fill">
              <ThemePreviewCanvas theme={theme} mode="light" />
            </StackItem>
            <StackItem size="fill">
              <ThemePreviewCanvas theme={theme} mode="dark" />
            </StackItem>
          </HStack>
        ) : (
          <ThemePreviewCanvas theme={theme} mode={scheme} />
        )}
      </VStack>
    </AspectRatio>
  );
}

const MODES: { value: Theme; label: MessageKey; icon: `site:${string}` }[] = [
  { value: "light", label: "settings.light", icon: "site:light" },
  { value: "dark", label: "settings.dark", icon: "site:dark" },
  { value: "system", label: "settings.system", icon: "site:system" },
];

/** The colour mode's cards. */
export function ThemeChoiceCards({
  value,
  onChange,
}: {
  value: Theme;
  onChange: (value: Theme) => void;
}) {
  const { t } = useUi();
  return (
    <Grid
      role="radiogroup"
      aria-label={t("settings.theme")}
      // the template's 96 px floor (page.tsx:1179) wraps System onto a row of its own in a phone's 264-294 px
      // panel; 80 keeps three across down to a 360 px screen
      columns={{ minWidth: 80, max: 3 }}
      gap={2}
      // the template's 30rem for three, so every card has the same width
      maxWidth="30rem"
    >
      {MODES.map((m) => {
        const isSelected = m.value === value;
        return (
          <SelectableCard
            key={m.value}
            label={t(m.label)}
            isSelected={isSelected}
            padding={2}
            // the selected card is passed on too, so a choice of the same mode is not saved again
            onChange={() => {
              if (!isSelected) onChange(m.value);
            }}
          >
            <VStack gap={1.5}>
              <ThemePreviewMock scheme={m.value} />
              <HStack gap={1} vAlign="center" hAlign="center">
                <Icon icon={m.icon} size="sm" color="secondary" />
                <Text
                  type="supporting"
                  weight={isSelected ? "semibold" : undefined}
                >
                  {t(m.label)}
                </Text>
              </HStack>
            </VStack>
          </SelectableCard>
        );
      })}
    </Grid>
  );
}
