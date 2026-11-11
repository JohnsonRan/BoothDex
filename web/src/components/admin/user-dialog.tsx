"use client";
// One account: GET /api/admin/users/{id}, in the Dialog the crawler history opens for a job (job-log.tsx, block
// Dialog — Scrolling content): the facts as the template incident-console's inspector lists them (MetadataList
// with the labels at the start, page.tsx:423-439), then the live sessions as its list rows (page.tsx:347-380,
// a ghost Sign out at each row's end), then the actions. Disabling asks first (block AlertDialogAsyncAction);
// admins come from ADMIN_USERNAMES, so they get a note instead of the button.

import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import {
  HStack,
  Layout,
  LayoutContent,
  VStack,
} from "@astryxdesign/core/Layout";
import { List, ListItem } from "@astryxdesign/core/List";
import {
  MetadataList,
  MetadataListItem,
} from "@astryxdesign/core/MetadataList";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Heading, Text } from "@astryxdesign/core/Text";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { Token } from "@astryxdesign/core/Token";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import { deviceName } from "@/lib/device";
import { errorText, useAdminT } from "./messages";
import { ResetPassword } from "./reset-password";
import type { AdminUser, AdminUserDetail } from "./types";

/** Admin and disabled marks, in a row's end and the dialog's header. */
export function UserTokens({ user }: { user: AdminUser }) {
  const t = useAdminT();
  return (
    <HStack gap={1}>
      {user.is_admin && (
        <Token size="sm" color="blue" label={t("users.admin")} />
      )}
      {user.disabled_at && (
        <Token size="sm" color="red" label={t("users.disabled")} />
      )}
    </HStack>
  );
}

export function UserDialog({
  id,
  onClose,
  onChange,
}: {
  id: number;
  onClose: () => void;
  /** the account changed: the list reads its page again */
  onChange: () => void;
}) {
  const t = useAdminT();
  const toast = useToast();
  const [user, setUser] = useState<AdminUserDetail>();
  const [error, setError] = useState<unknown>();
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = useCallback(
    () =>
      api<AdminUserDetail>(`/api/admin/users/${id}`).then((u) => {
        setUser(u);
        setError(undefined);
      }, setError),
    [id],
  );
  useEffect(() => {
    load();
  }, [load]);

  const act = async (fn: () => Promise<unknown>, done: string) => {
    setBusy(true);
    try {
      await fn();
      toast({ body: done });
      await load();
      onChange();
    } catch (e) {
      toast({ type: "error", body: errorText(t, e) });
    } finally {
      setBusy(false);
      setConfirm(false);
    }
  };
  const setDisabled = (u: AdminUserDetail, disabled: boolean) =>
    act(
      () =>
        api(`/api/admin/users/${u.id}`, {
          method: "PATCH",
          json: { disabled },
        }),
      t(disabled ? "users.disabledToast" : "users.enabledToast", {
        name: u.username,
      }),
    );

  return (
    <Dialog isOpen onOpenChange={onClose} width={720} maxHeight="85dvh">
      <Layout
        header={
          <DialogHeader
            title={user ? user.display_name : `#${id}`}
            subtitle={user && `@${user.username}`}
            endContent={user && <UserTokens user={user} />}
            onOpenChange={onClose}
          />
        }
        content={
          <LayoutContent>
            {error ? (
              <Banner status="error" title={errorText(t, error)} />
            ) : !user ? (
              <Spinner label={t("nav.users")} />
            ) : (
              <VStack gap={6} hAlign="stretch">
                <MetadataList
                  columns="single"
                  label={{ position: "start", width: 96 }}
                >
                  <MetadataListItem label={t("users.email")}>
                    <Text type="body">{user.email}</Text>
                  </MetadataListItem>
                  <MetadataListItem label={t("users.joined")}>
                    <Timestamp
                      value={user.created_at}
                      format="date_time"
                      type="body"
                    />
                  </MetadataListItem>
                  <MetadataListItem label={t("users.lastSignIn")}>
                    {user.last_sign_in_at ? (
                      <Timestamp
                        value={user.last_sign_in_at}
                        format="date_time"
                        type="body"
                      />
                    ) : (
                      <Text type="body" color="secondary">
                        {t("users.noRecord")}
                      </Text>
                    )}
                  </MetadataListItem>
                  <MetadataListItem label={t("users.data")}>
                    <Text type="body">
                      {t("users.dataValue", {
                        fav: user.favourites,
                        saved: user.saved_searches,
                        views: user.views,
                      })}
                    </Text>
                  </MetadataListItem>
                </MetadataList>

                <VStack gap={2}>
                  <Heading level={4} accessibilityLevel={3}>
                    {t("users.sessions")}
                  </Heading>
                  {user.sessions.length === 0 ? (
                    <Text type="supporting" color="secondary">
                      {t("users.noSessions")}
                    </Text>
                  ) : (
                    <List density="compact" hasDividers edge="outset">
                      {user.sessions.map((s) => (
                        <ListItem
                          key={s.id}
                          label={
                            deviceName(s.user_agent) ?? t("users.unknownDevice")
                          }
                          description={
                            <>
                              {s.ip && `${s.ip} · `}
                              <Timestamp
                                value={s.created_at}
                                format="relative"
                                type="supporting"
                                color="secondary"
                              />
                            </>
                          }
                          endContent={
                            <Button
                              label={t("users.revoke")}
                              variant="ghost"
                              size="sm"
                              isDisabled={busy}
                              onClick={() =>
                                act(
                                  () =>
                                    api(
                                      `/api/admin/users/${user.id}/sessions/${s.id}`,
                                      { method: "DELETE" },
                                    ),
                                  t("users.revoked"),
                                )
                              }
                            />
                          }
                        />
                      ))}
                    </List>
                  )}
                </VStack>

                <ResetPassword username={user.username} onReset={load}>
                  {user.is_admin ? null : user.disabled_at ? (
                    <Button
                      label={t("users.enable")}
                      variant="secondary"
                      isLoading={busy}
                      onClick={() => setDisabled(user, false)}
                    />
                  ) : (
                    <Button
                      label={t("users.disable")}
                      variant="destructive"
                      onClick={() => setConfirm(true)}
                    />
                  )}
                </ResetPassword>
                {user.is_admin && (
                  <Text type="supporting" color="secondary">
                    {t("error.adminDisable")}
                  </Text>
                )}
                <AlertDialog
                  isOpen={confirm}
                  onOpenChange={setConfirm}
                  title={t("users.disableTitle", { name: user.username })}
                  description={t("users.disableDesc")}
                  actionLabel={t("users.disable")}
                  actionVariant="destructive"
                  isActionLoading={busy}
                  onAction={() => setDisabled(user, true)}
                />
              </VStack>
            )}
          </LayoutContent>
        }
      />
    </Dialog>
  );
}
