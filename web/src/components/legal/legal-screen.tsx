// The privacy policy and the terms of use (lib/legal.ts), adapted from the Astryx page template
// documentation-technical (templates/pages/documentation-technical): a reading column of contentWidth 960, the
// title with its "Last updated" line (page.tsx:79-84), then each section as a level-2 heading over its paragraphs
// and disc lists (:146-154), a Divider between sections. The template's outline panel and its phone selector,
// its assistant card and code blocks are not used: the texts are short and have no code. The cookie list is a
// Table. Server-rendered in the site frame; the texts never reach the browser's bundle. The body (legal-body.tsx)
// is a client component fed plain data, so the dialog over the site (legal-dialog.tsx) fetches the same data
// through a server action (legal-action.ts) and renders it the same way.

import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import type { Metadata } from "next";
import { FilterUrlProvider } from "@/components/catalog/filter-url";
import { SiteFrame } from "@/components/frame/site-frame";
import { Providers } from "@/components/providers";
import { DEFAULT_FILTERS } from "@/lib/catalog";
import { MESSAGES, translator } from "@/lib/dictionaries";
import {
  contactEmail,
  LEGAL_UPDATED,
  type LegalKind,
  type LegalView,
  legalTexts,
  operatorName,
  SOURCE_URL,
} from "@/lib/legal";
import { en } from "@/lib/messages-en";
import { componentCatalogue, readSettings } from "@/lib/server-data";
import { LOCALES } from "@/lib/settings-core";
import { LegalBody } from "./legal-body";

/** In English, as the home page's: metadata read from the settings cookie leaves the route without a static
 * shell, which Next's dev server reports as an invariant error. */
export function legalMetadata(kind: LegalKind): Metadata {
  return {
    title: `${en[`legal.${kind}`]} · BoothDex`,
    description: en[`legal.${kind}Desc`],
  };
}

/** One text in the visitor's language, with the "Last updated" line and what its placeholders stand for. */
export async function legalDoc(kind: LegalKind): Promise<LegalView> {
  const settings = await readSettings();
  const t = translator(settings.ui);
  const texts = await legalTexts(settings.ui);
  const updated = new Intl.DateTimeFormat(LOCALES[settings.ui], {
    dateStyle: "long",
    timeZone: "UTC",
  }).format(new Date(LEGAL_UPDATED));
  return {
    ...texts[kind],
    updated: t("legal.updated", { date: updated }),
    fill: {
      operator: operatorName(),
      contact: contactEmail(),
      noContact: texts.noContact,
      source: SOURCE_URL,
      privacy: t("legal.privacy"),
    },
  };
}

export async function LegalScreen({ kind }: { kind: LegalKind }) {
  const settings = await readSettings();
  const doc = await legalDoc(kind);
  return (
    <Providers
      settings={settings}
      messages={MESSAGES[settings.ui]}
      catalogue={await componentCatalogue(settings.ui)}
    >
      {/* the account menu's views (favourites, recently viewed) open on the home page from here */}
      <FilterUrlProvider filters={DEFAULT_FILTERS}>
        <SiteFrame rates={Promise.resolve({})}>
          <Layout
            height="auto"
            contentWidth={960}
            content={
              <LayoutContent isScrollable={false} padding={6}>
                <VStack gap={8}>
                  <VStack gap={2}>
                    <Heading level={1}>{doc.title}</Heading>
                    <Text type="supporting" color="secondary">
                      {doc.updated}
                    </Text>
                  </VStack>
                  <LegalBody doc={doc} />
                </VStack>
              </LayoutContent>
            }
          />
        </SiteFrame>
      </FilterUrlProvider>
    </Providers>
  );
}
