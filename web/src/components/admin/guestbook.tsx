"use client";
// Guestbook moderation: GET /api/admin/guestbook, newest first, hidden messages included, 50 at a time with Load
// more, as the public drawer pages them. Rows as the users page (users.tsx): a List with dividers, the nickname
// and the text as typed, a Token for a hidden message, one for a closed message and a relative Timestamp at the
// end. Hide and Show act at once and can be undone; Delete cannot, so it asks first (AlertDialog, as disabling a
// user does). Close offers "as completed" and "as not planned" and a closed message offers Reopen, as GitHub's
// issue page does (https://docs.github.com/en/issues/tracking-your-work-with-issues/administering-issues/closing-an-issue).

import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { Button } from "@astryxdesign/core/Button";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { List, ListItem } from "@astryxdesign/core/List";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Text } from "@astryxdesign/core/Text";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { Token } from "@astryxdesign/core/Token";
import { useCallback, useEffect, useState } from "react";
import { MESSAGE_STYLE } from "@/components/guestbook/message-style";
import { CLOSED_COLOR } from "@/components/guestbook/status";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import type { GuestbookStatus } from "@/lib/api-types";
import { errorText, useAdminT } from "./messages";
import { PageFrame } from "./page-frame";
import type { AdminGuestbookEntry } from "./types";

type Page = { entries: AdminGuestbookEntry[]; more: boolean };
/** The action in flight: loading more, or one of a row's three. */
type Busy = "more" | `${number}:${"hidden" | "status" | "delete"}`;

function Entries() {
  const t = useAdminT();
  const toast = useToast();
  const [page, setPage] = useState<Page>();
  const [error, setError] = useState<unknown>();
  const [busy, setBusy] = useState<Busy>();
  const [deleting, setDeleting] = useState<AdminGuestbookEntry>();

  const load = useCallback(() => {
    setError(undefined);
    api<Page>("/api/admin/guestbook").then(setPage, setError);
  }, []);
  useEffect(() => load(), [load]);

  const act = async (key: Busy, fn: () => Promise<void>) => {
    setBusy(key);
    try {
      await fn();
    } catch (e) {
      toast({ type: "error", body: errorText(t, e) });
    } finally {
      setBusy(undefined);
    }
  };
  const more = () =>
    act("more", async () => {
      const last = page?.entries.at(-1);
      if (!last) return;
      const next = await api<Page>(`/api/admin/guestbook?before=${last.id}`);
      setPage(
        (p) =>
          p && { entries: [...p.entries, ...next.entries], more: next.more },
      );
    });
  const change = (
    e: AdminGuestbookEntry,
    key: "hidden" | "status",
    json: { hidden: boolean } | { status: GuestbookStatus },
  ) =>
    act(`${e.id}:${key}`, async () => {
      const r = await api<Pick<AdminGuestbookEntry, "hidden_at" | "status">>(
        `/api/admin/guestbook/${e.id}`,
        { method: "PATCH", json },
      );
      setPage(
        (p) =>
          p && {
            ...p,
            entries: p.entries.map((x) =>
              x.id === e.id
                ? { ...x, hidden_at: r.hidden_at, status: r.status }
                : x,
            ),
          },
      );
      if (key === "hidden")
        toast({
          body: t(r.hidden_at ? "guestbook.hidden" : "guestbook.shown"),
        });
      else
        toast({
          body: t(
            r.status === "open" ? "guestbook.reopened" : "guestbook.closed",
          ),
        });
    });
  const remove = (e: AdminGuestbookEntry) =>
    act(`${e.id}:delete`, async () => {
      await api(`/api/admin/guestbook/${e.id}`, { method: "DELETE" });
      setPage(
        (p) => p && { ...p, entries: p.entries.filter((x) => x.id !== e.id) },
      );
      setDeleting(undefined);
      toast({ body: t("guestbook.deleted") });
    });

  if (error)
    return (
      <EmptyState
        title={errorText(t, error)}
        icon={<Icon icon="admin:failed" size="lg" />}
        actions={<Button label={t("gate.retry")} onClick={load} />}
      />
    );
  if (!page) return <Spinner label={t("nav.guestbook")} />;
  if (page.entries.length === 0)
    return (
      <EmptyState
        title={t("guestbook.empty")}
        icon={<Icon icon="admin:empty" size="lg" />}
      />
    );
  return (
    <>
      <List density="compact" hasDividers edge="outset">
        {page.entries.map((e) => (
          <ListItem
            key={e.id}
            label={e.nickname}
            description={
              <Text type="supporting" color="secondary" style={MESSAGE_STYLE}>
                {e.body}
              </Text>
            }
            endContent={
              <HStack gap={3} vAlign="center">
                {e.hidden_at && (
                  <Token size="sm" label={t("guestbook.hiddenMark")} />
                )}
                {e.status !== "open" && (
                  <Token
                    size="sm"
                    color={CLOSED_COLOR[e.status]}
                    label={t(`guestbook.status.${e.status}`)}
                  />
                )}
                <Timestamp value={e.created_at} format="relative" />
                {e.status === "open" ? (
                  <DropdownMenu
                    button={{
                      label: t("guestbook.close"),
                      variant: "secondary",
                      size: "sm",
                      isLoading: busy === `${e.id}:status`,
                      isDisabled:
                        busy !== undefined && busy !== `${e.id}:status`,
                    }}
                    alignment="end"
                    menuWidth="max-content"
                    items={[
                      {
                        label: t("guestbook.closeCompleted"),
                        onClick: () =>
                          change(e, "status", { status: "completed" }),
                      },
                      {
                        label: t("guestbook.closeNotPlanned"),
                        onClick: () =>
                          change(e, "status", { status: "not_planned" }),
                      },
                    ]}
                  />
                ) : (
                  <Button
                    label={t("guestbook.reopen")}
                    variant="secondary"
                    size="sm"
                    isLoading={busy === `${e.id}:status`}
                    isDisabled={busy !== undefined && busy !== `${e.id}:status`}
                    onClick={() => change(e, "status", { status: "open" })}
                  />
                )}
                <Button
                  label={t(e.hidden_at ? "guestbook.show" : "guestbook.hide")}
                  variant="secondary"
                  size="sm"
                  isLoading={busy === `${e.id}:hidden`}
                  isDisabled={busy !== undefined && busy !== `${e.id}:hidden`}
                  onClick={() => change(e, "hidden", { hidden: !e.hidden_at })}
                />
                <Button
                  label={t("guestbook.delete")}
                  variant="destructive"
                  size="sm"
                  isDisabled={busy !== undefined}
                  onClick={() => setDeleting(e)}
                />
              </HStack>
            }
          />
        ))}
      </List>
      {page.more && (
        <VStack hAlign="center">
          <Button
            label={t("guestbook.more")}
            variant="secondary"
            isLoading={busy === "more"}
            onClick={more}
          />
        </VStack>
      )}
      <AlertDialog
        isOpen={deleting !== undefined}
        onOpenChange={(open) =>
          !open && busy === undefined && setDeleting(undefined)
        }
        title={t("guestbook.deleteTitle", { name: deleting?.nickname ?? "" })}
        description={t("guestbook.deleteDesc")}
        actionLabel={t("guestbook.deleteAction")}
        actionVariant="destructive"
        isActionLoading={
          deleting !== undefined && busy === `${deleting.id}:delete`
        }
        onAction={() => deleting && remove(deleting)}
      />
    </>
  );
}

export function GuestbookPage() {
  const t = useAdminT();
  return (
    <PageFrame title={t("nav.guestbook")}>
      <Entries />
    </PageFrame>
  );
}
