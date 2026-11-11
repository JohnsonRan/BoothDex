"use client";
// Reset one user's password from their details (master reset-password-card.tsx): the button, after the row's
// other actions, asks first through an AlertDialog; the new password is shown once in a success Banner under the
// row, with Copy and Hide.

import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { useState } from "react";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import { copyText } from "@/lib/clipboard";
import { errorText, useAdminT } from "./messages";

type Result = { username: string; password: string; sessions_revoked: number };

export function ResetPassword({
  username,
  onReset,
  children,
}: {
  username: string;
  /** the user's sessions are gone: read the details again */
  onReset: () => void;
  /** the row's other actions, before this one */
  children?: React.ReactNode;
}) {
  const t = useAdminT();
  const toast = useToast();
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Result>();

  const reset = async () => {
    setBusy(true);
    try {
      setResult(
        await api<Result>("/api/admin/users/reset-password", {
          method: "POST",
          json: { username },
        }),
      );
      onReset();
    } catch (e) {
      toast({ type: "error", body: errorText(t, e) });
    } finally {
      setBusy(false);
      setConfirm(false);
    }
  };

  return (
    <VStack gap={3}>
      <HStack gap={2} wrap="wrap">
        {children}
        <Button
          label={t("reset.submit")}
          variant="secondary"
          onClick={() => setConfirm(true)}
        />
      </HStack>
      {result && (
        <Banner
          status="success"
          title={t("reset.result", { name: result.username })}
          description={
            <VStack gap={2}>
              <Text type="code">{result.password}</Text>
              <HStack gap={2}>
                <Button
                  label={t("reset.copy")}
                  variant="secondary"
                  size="sm"
                  icon={<Icon icon="admin:copy" size="sm" />}
                  onClick={() =>
                    copyText(result.password).then(
                      () => toast({ body: t("reset.copied") }),
                      () =>
                        toast({ type: "error", body: t("reset.copyFailed") }),
                    )
                  }
                />
                <Button
                  label={t("reset.hide")}
                  variant="ghost"
                  size="sm"
                  onClick={() => setResult(undefined)}
                />
              </HStack>
              <Text type="supporting" color="secondary">
                {t("reset.revoked", { n: result.sessions_revoked })}
              </Text>
            </VStack>
          }
        />
      )}
      <AlertDialog
        isOpen={confirm}
        onOpenChange={setConfirm}
        title={t("reset.confirmTitle", { name: username })}
        description={t("reset.confirmDesc")}
        actionLabel={t("reset.submit")}
        actionVariant="destructive"
        isActionLoading={busy}
        onAction={reset}
      />
    </VStack>
  );
}
