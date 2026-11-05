"use client";
// Reset a user's password (master reset-password-card.tsx): the two-click arm button becomes an AlertDialog.
// The new password is shown once in a success Banner with Copy and Hide; an unknown name is the field's error.

import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useToast } from "@astryxdesign/core/Toast";
import { useState } from "react";
import { ApiError, api } from "@/lib/api";
import { useAdminT } from "./messages";

type Result = { username: string; password: string; sessions_revoked: number };

export function ResetPassword() {
  const t = useAdminT();
  const toast = useToast();
  const [username, setUsername] = useState("");
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notFound, setNotFound] = useState(false);
  const [result, setResult] = useState<Result>();
  const name = username.trim();

  const reset = async () => {
    setBusy(true);
    try {
      setResult(
        await api<Result>("/api/admin/users/reset-password", {
          method: "POST",
          json: { username: name },
        }),
      );
      setUsername("");
    } catch (e) {
      if (e instanceof ApiError && e.status === 404) setNotFound(true);
      else
        toast({
          type: "error",
          body: e instanceof Error ? e.message : String(e),
        });
    } finally {
      setBusy(false);
      setConfirm(false);
    }
  };

  return (
    <VStack gap={4}>
      <TextInput
        label={t("reset.username")}
        autoComplete="off"
        value={username}
        onChange={(v) => {
          setUsername(v);
          setNotFound(false);
        }}
        status={
          notFound ? { type: "error", message: t("reset.notFound") } : undefined
        }
      />
      <HStack>
        <Button
          label={t("reset.submit")}
          variant="destructive"
          isDisabled={!name}
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
                    navigator.clipboard.writeText(result.password).then(
                      () => toast({ body: t("reset.copied") }),
                      (e: Error) => toast({ type: "error", body: e.message }),
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
        title={t("reset.confirmTitle", { name })}
        description={t("reset.confirmDesc")}
        actionLabel={t("reset.submit")}
        actionVariant="destructive"
        isActionLoading={busy}
        onAction={reset}
      />
    </VStack>
  );
}
