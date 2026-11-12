"use client";
// Settings > Your data: the template settings-dialog's "Delete my account" row (page.tsx:1953-1958, a destructive
// sm Button) opening a FormDialog. The API asks for the password (crates/api/src/account/profile.rs delete_account) and
// deletes the account with its sessions, favourites, viewed items and saved searches; the session then reads as
// signed out, as after Sign out.

import { Button } from "@astryxdesign/core/Button";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useState } from "react";
import { useAccount } from "@/components/account/session";
import { useUi } from "@/components/providers";
import { useToast } from "@/components/toasts";
import { api, errorKey } from "@/lib/api";
import { API_ERRORS, type MessageKey } from "@/lib/messages";
import { FormDialog } from "./form-dialog";
import { SettingsCard, SettingsRow } from "./rows";

export function DataPanel() {
  const { t } = useUi();
  const { me } = useAccount();
  const [open, setOpen] = useState(false);
  // the API refuses it too: a deleted admin's name would make whoever registers it next an admin
  const isAdmin = !!me?.is_admin;
  return (
    <SettingsCard>
      <SettingsRow
        title={t("delete.title")}
        icon="site:clear"
        description={t(isAdmin ? "delete.admin" : "delete.desc")}
        control={
          <Button
            label={t("delete.button")}
            variant="destructive"
            size="sm"
            isDisabled={isAdmin}
            onClick={() => setOpen(true)}
          />
        }
      />
      {open && <DeleteDialog onClose={() => setOpen(false)} />}
    </SettingsCard>
  );
}

function DeleteDialog({ onClose }: { onClose: () => void }) {
  const { t } = useUi();
  const toast = useToast();
  const { signOut } = useAccount();
  const [password, setPassword] = useState("");
  // the password the API refused; its error shows on the field until the field changes, as in Profile
  const [refused, setRefused] = useState<string>();
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const problem: MessageKey | undefined = !password
    ? "account.required"
    : password === refused
      ? "error.password"
      : undefined;

  const submit = async () => {
    setChecked(true);
    if (problem) return;
    setBusy(true);
    try {
      await api("/api/account", { method: "DELETE", json: { password } });
    } catch (e) {
      setBusy(false);
      const key = errorKey(e, API_ERRORS, "error.failed");
      if (key === "error.password") return setRefused(password);
      return toast({ type: "error", body: t(key), uniqueID: "delete" });
    }
    toast({ body: t("delete.done"), uniqueID: "delete" });
    onClose();
    // the cookie is already cleared; this reads the session again and leaves account-only views
    await signOut();
  };

  return (
    <FormDialog
      title={t("delete.title")}
      subtitle={t("delete.confirm")}
      action={t("delete.title")}
      isDestructive
      isBusy={busy}
      onClose={onClose}
      onSubmit={submit}
    >
      <TextInput
        label={t("account.password")}
        type="password"
        autoComplete="current-password"
        value={password}
        onChange={setPassword}
        status={
          checked && problem
            ? { type: "error", message: t(problem) }
            : undefined
        }
      />
    </FormDialog>
  );
}
