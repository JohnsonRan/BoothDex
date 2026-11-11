"use client";
// Settings, from the Astryx page template settings (templates/pages/settings/page.tsx): one scrolling form of
// sections, each a two-column Grid (:120-150) with the heading and supporting text on the left and the fields
// with their own Save on the right, a Divider between sections. The template's 40 px gap parts the columns; when
// a phone stacks them, the fields follow the description at the fields' own 16 px gap. With two sections the template's side list and
// search are not needed.

import { Divider } from "@astryxdesign/core/Divider";
import { Grid } from "@astryxdesign/core/Grid";
import { VStack } from "@astryxdesign/core/Layout";
import { Heading, Text } from "@astryxdesign/core/Text";
import { AiSettings } from "./ai-settings";
import { useAdminT } from "./messages";
import { PageFrame } from "./page-frame";
import { Registration } from "./registration";

function Section({
  title,
  desc,
  children,
}: {
  title: string;
  desc: string;
  children: React.ReactNode;
}) {
  return (
    <Grid columns={{ minWidth: 320 }} columnGap={10} rowGap={4}>
      <VStack gap={1}>
        <Heading level={3} accessibilityLevel={2}>
          {title}
        </Heading>
        <Text type="supporting" color="secondary">
          {desc}
        </Text>
      </VStack>
      <VStack gap={4}>{children}</VStack>
    </Grid>
  );
}

export function SettingsPage() {
  const t = useAdminT();
  return (
    <PageFrame title={t("settings.title")}>
      <Section title={t("ai.title")} desc={t("ai.desc")}>
        <AiSettings />
      </Section>
      <Divider />
      <Section title={t("registration.title")} desc={t("registration.desc")}>
        <Registration />
      </Section>
    </PageFrame>
  );
}
