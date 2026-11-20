"use client";
// The guestbook, a Drawer from the end edge (Astryx DrawerForm: DrawerHeader with its close button, the body in
// LayoutContent): the form on top, then the messages, newest first, 50 at a time with Load more under them, each
// as a comment thread shows one (GitHub, YouTube): nickname, relative time with the full time on hover and focus,
// the text as typed, and a mark on one an admin closed (completed or not planned, as a closed issue carries).
// Text is rendered as text only, never as HTML or links. The first page shows a skeleton of its rows; a reopen
// reads the page again behind what is shown. While either Turnstile key is unset the API answers no site key,
// and the form's place shows that the guestbook is not open yet. A click outside closes it, as Escape does: the
// form stays mounted while the drawer is closed, so a draft is there again on the next open.

import { Button } from "@astryxdesign/core/Button";
import { Divider } from "@astryxdesign/core/Divider";
import { Drawer, DrawerHeader } from "@astryxdesign/core/Drawer";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import {
  HStack,
  Layout,
  LayoutContent,
  VStack,
} from "@astryxdesign/core/Layout";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { Text } from "@astryxdesign/core/Text";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { Token } from "@astryxdesign/core/Token";
import { Fragment, useCallback, useEffect, useState } from "react";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import type { GuestbookEntry, GuestbookPage } from "@/lib/api-types";
import { MESSAGE_STYLE } from "./message-style";
import { guestbookError, useGuestbookT } from "./messages";
import { PostForm } from "./post-form";
import { CLOSED_COLOR } from "./status";

const SKELETON_ROWS = 3;
/** one line of the given text type, the height a skeleton row's line takes */
const line = (type: "label" | "body") =>
  `calc(var(--text-${type}-size) * var(--text-${type}-leading))`;

function Entry({ entry }: { entry: GuestbookEntry }) {
  const t = useGuestbookT();
  return (
    <VStack gap={1}>
      <HStack gap={2} vAlign="center" wrap="wrap">
        <Text type="label" wordBreak="break-word">
          {entry.nickname}
        </Text>
        <Timestamp value={entry.created_at} format="relative" />
        {entry.status !== "open" && (
          <Token
            size="sm"
            color={CLOSED_COLOR[entry.status]}
            label={t(`status.${entry.status}`)}
          />
        )}
      </HStack>
      <Text type="body" as="p" style={MESSAGE_STYLE}>
        {entry.body}
      </Text>
    </VStack>
  );
}

function Rows({ children }: { children: React.ReactElement[] }) {
  return (
    <VStack gap={4}>
      {children.map((row, i) => (
        <Fragment key={row.key}>
          {i > 0 && <Divider />}
          {row}
        </Fragment>
      ))}
    </VStack>
  );
}

export function GuestbookDrawer({
  isOpen,
  onOpenChange,
}: {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const t = useGuestbookT();
  const toast = useToast();
  const [page, setPage] = useState<GuestbookPage>();
  const [failed, setFailed] = useState(false);
  const [moreBusy, setMoreBusy] = useState(false);

  const load = useCallback(
    () =>
      api<GuestbookPage>("/api/guestbook").then(
        (p) => {
          setPage(p);
          setFailed(false);
        },
        () => setFailed(true),
      ),
    [],
  );
  useEffect(() => {
    if (isOpen) load();
  }, [isOpen, load]);

  const loadMore = async () => {
    const last = page?.entries.at(-1);
    if (!last) return;
    setMoreBusy(true);
    try {
      const next = await api<GuestbookPage>(`/api/guestbook?before=${last.id}`);
      setPage(
        (p) =>
          p && {
            ...p,
            entries: [...p.entries, ...next.entries],
            more: next.more,
          },
      );
    } catch (e) {
      toast({
        type: "error",
        body: t(guestbookError(e)),
        uniqueID: "guestbook",
      });
    } finally {
      setMoreBusy(false);
    }
  };
  const posted = (entry: GuestbookEntry) =>
    setPage((p) => p && { ...p, entries: [entry, ...p.entries] });
  // the form gives way to the closed state, as the next open would show it
  const closed = () => setPage((p) => p && { ...p, site_key: null });

  let body: React.ReactNode;
  if (!page && failed)
    body = (
      <EmptyState
        headingLevel={3}
        title={t("failed")}
        icon={<Icon icon="error" size="lg" />}
        actions={
          <Button
            label={t("retry")}
            onClick={() => {
              setFailed(false);
              load();
            }}
          />
        }
      />
    );
  else if (!page)
    body = (
      <Rows>
        {Array.from({ length: SKELETON_ROWS }, (_, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: fixed placeholders, never reordered
          <VStack key={i} gap={1}>
            <Skeleton width="8rem" height={line("label")} index={i} />
            <Skeleton height={line("body")} index={i} />
          </VStack>
        ))}
      </Rows>
    );
  else
    body = (
      <VStack gap={6}>
        {page.site_key ? (
          <PostForm
            siteKey={page.site_key}
            onPosted={posted}
            onClosed={closed}
          />
        ) : (
          <EmptyState
            headingLevel={3}
            title={t("closed")}
            description={t("closedDesc")}
            icon={<Icon icon="site:guestbook" size="lg" />}
          />
        )}
        {page.entries.length > 0 ? (
          <>
            <Divider />
            <Rows>
              {page.entries.map((e) => (
                <Entry key={e.id} entry={e} />
              ))}
            </Rows>
            {page.more && (
              <Button
                label={t("more")}
                variant="secondary"
                width="100%"
                isLoading={moreBusy}
                onClick={loadMore}
              />
            )}
          </>
        ) : (
          page.site_key && (
            <EmptyState
              headingLevel={3}
              title={t("empty")}
              description={t("emptyDesc")}
              icon={<Icon icon="site:guestbook" size="lg" />}
            />
          )
        )}
      </VStack>
    );

  return (
    <Drawer
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      label={t("title")}
      purpose="info"
      width={440}
    >
      <Layout
        header={
          <DrawerHeader
            title={t("title")}
            subtitle={t("subtitle")}
            onOpenChange={onOpenChange}
          />
        }
        content={<LayoutContent>{body}</LayoutContent>}
      />
    </Drawer>
  );
}
