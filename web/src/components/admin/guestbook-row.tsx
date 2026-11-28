"use client";
// One row of the admin guestbook moderation list (guestbook.tsx).

import { Button } from "@astryxdesign/core/Button";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { HStack } from "@astryxdesign/core/Layout";
import { ListItem } from "@astryxdesign/core/List";
import { Text } from "@astryxdesign/core/Text";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { Token } from "@astryxdesign/core/Token";
import { type RefObject, useCallback } from "react";
import { MESSAGE_STYLE } from "@/components/guestbook/message-style";
import { CLOSED_COLOR } from "@/components/guestbook/status";
import type { GuestbookStatus } from "@/lib/api-types";
import type { AdminT } from "./messages";
import type { AdminGuestbookEntry } from "./types";

/** The action in flight: loading more, or one of a row's three. */
export type Busy = "more" | `${number}:${"hidden" | "status" | "delete"}`;

export function GuestbookRow({
  entry: e,
  busy,
  refocus,
  t,
  onChange,
  onDelete,
}: {
  entry: AdminGuestbookEntry;
  busy: Busy | undefined;
  refocus: RefObject<number | null>;
  t: AdminT;
  onChange: (
    entry: AdminGuestbookEntry,
    key: "hidden" | "status",
    json: { hidden: boolean } | { status: GuestbookStatus },
  ) => void;
  onDelete: (entry: AdminGuestbookEntry) => void;
}) {
  const statusRef = useCallback(
    (el: HTMLButtonElement | null) => {
      if (el && refocus.current === e.id) {
        refocus.current = null;
        el.focus();
      }
    },
    [e.id, refocus],
  );
  return (
    <ListItem
      label={e.nickname}
      description={
        <Text type="supporting" color="secondary" style={MESSAGE_STYLE}>
          {e.body}
        </Text>
      }
      endContent={
        <HStack gap={3} vAlign="center">
          {e.hidden_at && <Token size="sm" label={t("guestbook.hiddenMark")} />}
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
                ref: statusRef,
                label: `${t("guestbook.close")} ${e.nickname}`,
                children: t("guestbook.close"),
                variant: "secondary",
                size: "sm",
                isLoading: busy === `${e.id}:status`,
                isDisabled: busy !== undefined && busy !== `${e.id}:status`,
              }}
              alignment="end"
              menuWidth="max-content"
              items={[
                {
                  label: t("guestbook.closeCompleted"),
                  onClick: () => onChange(e, "status", { status: "completed" }),
                },
                {
                  label: t("guestbook.closeNotPlanned"),
                  onClick: () =>
                    onChange(e, "status", { status: "not_planned" }),
                },
              ]}
            />
          ) : (
            <Button
              ref={statusRef}
              label={`${t("guestbook.reopen")} ${e.nickname}`}
              variant="secondary"
              size="sm"
              isLoading={busy === `${e.id}:status`}
              isDisabled={busy !== undefined && busy !== `${e.id}:status`}
              onClick={() => onChange(e, "status", { status: "open" })}
            >
              {t("guestbook.reopen")}
            </Button>
          )}
          <Button
            label={`${t(e.hidden_at ? "guestbook.show" : "guestbook.hide")} ${e.nickname}`}
            variant="secondary"
            size="sm"
            isLoading={busy === `${e.id}:hidden`}
            isDisabled={busy !== undefined && busy !== `${e.id}:hidden`}
            onClick={() => onChange(e, "hidden", { hidden: !e.hidden_at })}
          >
            {t(e.hidden_at ? "guestbook.show" : "guestbook.hide")}
          </Button>
          <Button
            label={`${t("guestbook.delete")} ${e.nickname}`}
            variant="destructive"
            size="sm"
            isDisabled={busy !== undefined}
            onClick={() => onDelete(e)}
          >
            {t("guestbook.delete")}
          </Button>
        </HStack>
      }
    />
  );
}
