"use client";
// The appearance cards, copied from template settings-dialog (page.tsx:1058-1218: ThemePreviewCanvas,
// ThemePreviewMock, ThemeChoiceCards): a SelectableCard per choice with a miniature page drawn by a nested Theme,
// then its icon and name. The colour mode's three are drawn in the theme shown, each in its mode (System shows
// light and dark halves); the Astryx themes' are the template's cards again, as no template has a theme picker,
// each drawn in its own theme in the mode shown. The miniature's StyleX rules (page.tsx:294-322) are inline
// token styles here.

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
import { type CSSProperties, type ReactNode, useEffect, useState } from "react";
import { defaultThemeSlug, themes } from "@/astryx-themes";
import { useUi } from "@/components/providers";
import type { MessageKey } from "@/lib/messages";
import { STYLES, type Style, type Theme } from "@/lib/settings-core";
import { loadStyleTheme, styleTheme } from "@/themes";

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

/** A theme in a mode; until the theme is in, the frame alone, so the card keeps its size. */
function ThemePreviewMock({
  theme,
  scheme,
}: {
  theme: DefinedTheme | undefined;
  scheme: Theme;
}) {
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
        {!theme ? null : scheme === "system" ? (
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

function ChoiceCards<T extends string>({
  label,
  value,
  onChange,
  choices,
  max,
}: {
  label: string;
  value: T;
  onChange: (value: T) => void;
  choices: {
    value: T;
    label: string;
    icon?: `site:${string}`;
    preview: ReactNode;
  }[];
  /** cards across */
  max: number;
}) {
  return (
    <Grid
      role="radiogroup"
      aria-label={label}
      // the template's 96 px floor (page.tsx:1179) wraps System onto a row of its own in a phone's 264-294 px
      // panel; 80 keeps three across down to a 360 px screen
      columns={{ minWidth: 80, max }}
      gap={2}
      // the template's 30rem for three, so every card has the same width
      maxWidth={`${10 * max}rem`}
    >
      {choices.map((choice) => {
        const isSelected = choice.value === value;
        return (
          <SelectableCard
            key={choice.value}
            label={choice.label}
            isSelected={isSelected}
            padding={2}
            onChange={() => {
              if (!isSelected) onChange(choice.value);
            }}
          >
            <VStack gap={1.5}>
              {choice.preview}
              <HStack gap={1} vAlign="center" hAlign="center">
                {choice.icon && (
                  <Icon icon={choice.icon} size="sm" color="secondary" />
                )}
                <Text
                  type="supporting"
                  weight={isSelected ? "semibold" : undefined}
                >
                  {choice.label}
                </Text>
              </HStack>
            </VStack>
          </SelectableCard>
        );
      })}
    </Grid>
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
  const { t, settings } = useUi();
  // loaded already: it is the theme shown
  const theme = styleTheme(settings.style) ?? themes[defaultThemeSlug];
  return (
    <ChoiceCards
      label={t("settings.theme")}
      value={value}
      onChange={onChange}
      max={3}
      choices={MODES.map((m) => ({
        value: m.value,
        label: t(m.label),
        icon: m.icon,
        preview: <ThemePreviewMock theme={theme} scheme={m.value} />,
      }))}
    />
  );
}

/** The Astryx themes' cards; each theme loads for its preview, without its fonts (the miniature has no text). */
export function StyleChoiceCards({
  value,
  onChange,
}: {
  value: Style;
  onChange: (value: Style) => void;
}) {
  const { t, settings } = useUi();
  const [loaded, setLoaded] = useState<Partial<Record<Style, DefinedTheme>>>({
    neutral: themes[defaultThemeSlug],
  });
  useEffect(() => {
    for (const style of STYLES)
      loadStyleTheme(style).then((theme) => {
        if (theme) setLoaded((l) => ({ ...l, [style]: theme }));
      });
  }, []);
  return (
    <ChoiceCards
      label={t("settings.style")}
      value={value}
      onChange={onChange}
      max={4}
      choices={STYLES.map((style) => ({
        value: style,
        label: t(`style.${style}`),
        preview: (
          <ThemePreviewMock theme={loaded[style]} scheme={settings.theme} />
        ),
      }))}
    />
  );
}
