// The privacy policy and the terms of use (lib/legal.ts), adapted from the Astryx page template
// documentation-technical (templates/pages/documentation-technical): a reading column of contentWidth 960, the
// title with its "Last updated" line (page.tsx:79-84), then each section as a level-2 heading over its paragraphs
// and disc lists (:146-154), a Divider between sections. The template's outline panel and its phone selector,
// its assistant card and code blocks are not used: the texts are short and have no code. The cookie list is a
// Table. Server-rendered in the site frame; the texts never reach the browser's bundle. The same body opens in a
// dialog over the site (legal-dialog.tsx), fetched through a server action (legal-action.ts).

import { Divider } from "@astryxdesign/core/Divider";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { Link } from "@astryxdesign/core/Link";
import { List, ListItem } from "@astryxdesign/core/List";
import { VStack } from "@astryxdesign/core/Stack";
import { proportional, Table } from "@astryxdesign/core/Table";
import { Heading, Text } from "@astryxdesign/core/Text";
import type { Metadata } from "next";
import { Fragment, type ReactNode } from "react";
import { FilterUrlProvider } from "@/components/catalog/filter-url";
import { SiteFrame } from "@/components/frame/site-frame";
import { Providers } from "@/components/providers";
import { DEFAULT_FILTERS } from "@/lib/catalog";
import { MESSAGES, translator } from "@/lib/dictionaries";
import {
  contactEmail,
  LEGAL_UPDATED,
  type LegalBlock,
  type LegalKind,
  legalTexts,
  operatorName,
  SOURCE_URL,
} from "@/lib/legal";
import { en } from "@/lib/messages-en";
import { rich } from "@/lib/rich";
import { componentCatalogue, readSettings } from "@/lib/server-data";
import { LOCALES } from "@/lib/settings-core";

/** In English, as the home page's: metadata read from the settings cookie leaves the route without a static
 * shell, which Next's dev server reports as an invariant error. */
export function legalMetadata(kind: LegalKind): Metadata {
  return {
    title: `${en[`legal.${kind}`]} · BoothDex`,
    description: en[`legal.${kind}Desc`],
  };
}

function Block({
  block,
  nodes,
}: {
  block: LegalBlock;
  nodes: Record<string, ReactNode>;
}) {
  if (typeof block === "string")
    return (
      <Text as="p" type="body">
        {rich(block, nodes)}
      </Text>
    );
  if ("list" in block)
    return (
      <List density="compact" listStyle="disc">
        {block.list.map((item) => (
          <ListItem key={item} label={rich(item, nodes)} />
        ))}
      </List>
    );
  const { columns, rows } = block.table;
  return (
    <Table
      // the last two columns (purpose, period) hold sentences; the first two a word each
      columns={columns.map((header, i) => ({
        key: `c${i}`,
        header,
        width: proportional(i === columns.length - 2 ? 3 : i > 0 ? 2 : 1.5),
      }))}
      data={rows.map((row) =>
        Object.fromEntries(row.map((cell, i) => [`c${i}`, cell])),
      )}
      idKey="c0"
      density="compact"
      verticalAlign="top"
    />
  );
}

/** One text in the visitor's language: its title, the "Last updated" line and the body under them. */
export async function legalDoc(kind: LegalKind): Promise<{
  title: string;
  updated: string;
  body: ReactNode;
}> {
  const settings = await readSettings();
  const t = translator(settings.ui);
  const texts = await legalTexts(settings.ui);
  const doc = texts[kind];
  const email = contactEmail();
  // underlined: neutral's accent is the text colour, so inside a paragraph a link is told apart by its line
  const nodes: Record<string, ReactNode> = {
    operator: operatorName(),
    contact: email ? (
      <Link href={`mailto:${email}`} hasUnderline>
        {email}
      </Link>
    ) : (
      texts.noContact
    ),
    source: (
      <Link href={SOURCE_URL} target="_blank" rel="noopener" hasUnderline>
        {SOURCE_URL.replace("https://", "")}
      </Link>
    ),
    privacy: (
      <Link href="/privacy" hasUnderline>
        {t("legal.privacy")}
      </Link>
    ),
  };
  const updated = new Intl.DateTimeFormat(LOCALES[settings.ui], {
    dateStyle: "long",
    timeZone: "UTC",
  }).format(new Date(LEGAL_UPDATED));
  return {
    title: doc.title,
    updated: t("legal.updated", { date: updated }),
    body: (
      <VStack gap={8}>
        <Text as="p" type="body">
          {rich(doc.summary, nodes)}
        </Text>
        {doc.sections.map((section) => (
          <Fragment key={section.id}>
            <Divider />
            <VStack gap={4}>
              <Heading id={section.id} level={2}>
                {section.heading}
              </Heading>
              {section.body.map((block, i) => (
                <Block
                  // biome-ignore lint/suspicious/noArrayIndexKey: a fixed text, never reordered
                  key={i}
                  block={block}
                  nodes={nodes}
                />
              ))}
            </VStack>
          </Fragment>
        ))}
      </VStack>
    ),
  };
}

export async function LegalScreen({ kind }: { kind: LegalKind }) {
  const settings = await readSettings();
  const { title, updated, body } = await legalDoc(kind);
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
                    <Heading level={1}>{title}</Heading>
                    <Text type="supporting" color="secondary">
                      {updated}
                    </Text>
                  </VStack>
                  {body}
                </VStack>
              </LayoutContent>
            }
          />
        </SiteFrame>
      </FilterUrlProvider>
    </Providers>
  );
}
