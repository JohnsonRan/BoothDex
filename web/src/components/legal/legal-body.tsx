"use client";
// The body of the privacy policy or the terms (lib/legal.ts): each section as a level-2 heading over its
// paragraphs and disc lists, a Divider between sections, the cookie list a Table (see legal-screen.tsx). A client
// component fed plain data, so the page (legal-screen.tsx) and the dialog (legal-dialog.tsx) render the same
// thing, and the dialog's server action (legal-action.ts) returns data rather than elements: elements would name
// client components the calling route's manifest may not list, which only a production build enforces. The
// texts still reach the browser as props, not in its bundle.

import { Divider } from "@astryxdesign/core/Divider";
import { Link } from "@astryxdesign/core/Link";
import { List, ListItem } from "@astryxdesign/core/List";
import { VStack } from "@astryxdesign/core/Stack";
import { proportional, Table } from "@astryxdesign/core/Table";
import { Heading, Text } from "@astryxdesign/core/Text";
import { Fragment, type ReactNode } from "react";
import type { LegalBlock, LegalView } from "@/lib/legal";
import { rich } from "@/lib/rich";

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

export function LegalBody({ doc }: { doc: LegalView }) {
  const { operator, contact, noContact, source, privacy } = doc.fill;
  // underlined: neutral's accent is the text colour, so inside a paragraph a link is told apart by its line
  const nodes: Record<string, ReactNode> = {
    operator,
    contact: contact ? (
      <Link href={`mailto:${contact}`} hasUnderline>
        {contact}
      </Link>
    ) : (
      noContact
    ),
    source: (
      <Link href={source} target="_blank" rel="noopener" hasUnderline>
        {source.replace("https://", "")}
      </Link>
    ),
    privacy: (
      <Link href="/privacy" hasUnderline>
        {privacy}
      </Link>
    ),
  };
  return (
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
  );
}
