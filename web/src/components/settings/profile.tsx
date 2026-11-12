"use client";
// Settings > Profile: the display name and the password, each a row whose button opens a FormDialog, as "Delete
// account" in Your data. GitHub (Settings > Public profile; Password and authentication) and Google (Personal info;
// Security) keep both in the account's settings and ask for the current password and the new one twice. The API
// (crates/api/src/account/profile.rs update_profile, change_password) checks the current password under the sign-in
// limits and signs the account out of its other sessions. The username cannot change: ADMIN_USERNAMES names
// admins by it.

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

export function ProfilePanel() {
  const { t } = useUi();
  const { me } = useAccount();
  const [open, setOpen] = useState<"name" | "password">();
  const close = () => setOpen(undefined);
  return (
    <>
      <SettingsCard>
        <SettingsRow
          title={t("account.displayName")}
          icon="site:account"
          description={me?.display_name}
          control={
            <Button
              label={t("profile.nameTitle")}
              variant="secondary"
              size="sm"
              onClick={() => setOpen("name")}
            />
          }
        />
        <SettingsRow
          title={t("account.password")}
          icon="site:password"
          description={t("profile.passwordDesc")}
          control={
            <Button
              label={t("profile.passwordTitle")}
              variant="secondary"
              size="sm"
              onClick={() => setOpen("password")}
            />
          }
        />
      </SettingsCard>
      {open === "name" && <NameDialog onClose={close} />}
      {open === "password" && <PasswordDialog onClose={close} />}
    </>
  );
}

/**
 * Sends a change; on failure shows the API's reason, or `error.failed`, in a toast, unless `onField` shows it on a
 * field and returns true. True when it was saved.
 */
function useSave(onField?: (key: MessageKey) => boolean) {
  const { t } = useUi();
  const toast = useToast();
  const [busy, setBusy] = useState(false);
  const save = async (path: string, method: string, json: unknown) => {
    setBusy(true);
    try {
      await api(path, { method, json });
      return true;
    } catch (e) {
      const key = errorKey(e, API_ERRORS, "error.failed");
      if (!onField?.(key))
        toast({ type: "error", body: t(key), uniqueID: "profile" });
      return false;
    } finally {
      setBusy(false);
    }
  };
  return { busy, save };
}

function NameDialog({ onClose }: { onClose: () => void }) {
  const { t } = useUi();
  const toast = useToast();
  const { me, reload } = useAccount();
  const { busy, save } = useSave();
  const [name, setName] = useState(me?.display_name ?? "");
  const [checked, setChecked] = useState(false);
  // the API's rule: 1 to 50 characters once trimmed
  const length = [...name.trim()].length;
  const problem: MessageKey | undefined =
    length === 0
      ? "account.required"
      : length > 50
        ? "profile.nameLong"
        : undefined;

  const submit = async () => {
    setChecked(true);
    if (problem) return;
    if (
      !(await save("/api/account/profile", "PATCH", {
        display_name: name.trim(),
      }))
    )
      return;
    toast({ body: t("profile.nameSaved"), uniqueID: "profile" });
    onClose();
    reload();
  };

  return (
    <FormDialog
      title={t("profile.nameTitle")}
      action={t("saved.save")}
      isBusy={busy}
      onClose={onClose}
      onSubmit={submit}
    >
      <TextInput
        label={t("account.displayName")}
        autoComplete="nickname"
        value={name}
        onChange={setName}
        status={
          checked && problem
            ? { type: "error", message: t(problem) }
            : undefined
        }
      />
    </FormDialog>
  );
}

type PasswordField = "current" | "next" | "again";

const PASSWORD_FIELDS: {
  field: PasswordField;
  label: MessageKey;
  autoComplete: string;
}[] = [
  {
    field: "current",
    label: "profile.current",
    autoComplete: "current-password",
  },
  { field: "next", label: "profile.new", autoComplete: "new-password" },
  { field: "again", label: "profile.again", autoComplete: "new-password" },
];

function PasswordDialog({ onClose }: { onClose: () => void }) {
  const { t } = useUi();
  const toast = useToast();
  const [values, setValues] = useState<Record<PasswordField, string>>({
    current: "",
    next: "",
    again: "",
  });
  // the current password the API refused; its error shows on that field until the field changes
  const [refused, setRefused] = useState<string>();
  const { busy, save } = useSave((key) => {
    if (key !== "error.password") return false;
    setRefused(values.current);
    return true;
  });
  const [checked, setChecked] = useState(false);
  const problem = (field: PasswordField): MessageKey | undefined => {
    const v = values[field];
    if (!v) return "account.required";
    if (field === "current" && v === refused) return "error.password";
    if (field === "next" && v.length < 10) return "account.passwordShort";
    if (field === "next" && v === values.current) return "profile.same";
    if (field === "again" && v !== values.next) return "profile.mismatch";
  };

  const submit = async () => {
    setChecked(true);
    if (PASSWORD_FIELDS.some(({ field }) => problem(field))) return;
    const body = { current: values.current, new: values.next };
    if (!(await save("/api/account/password", "POST", body))) return;
    toast({ body: t("profile.passwordSaved"), uniqueID: "profile" });
    onClose();
  };

  return (
    <FormDialog
      title={t("profile.passwordTitle")}
      subtitle={t("profile.passwordDesc")}
      action={t("profile.passwordTitle")}
      isBusy={busy}
      onClose={onClose}
      onSubmit={submit}
    >
      {PASSWORD_FIELDS.map(({ field, label, autoComplete }) => {
        const key = checked ? problem(field) : undefined;
        return (
          <TextInput
            key={field}
            label={t(label)}
            type="password"
            autoComplete={autoComplete}
            value={values[field]}
            onChange={(v) => setValues((old) => ({ ...old, [field]: v }))}
            status={key ? { type: "error", message: t(key) } : undefined}
          />
        );
      })}
    </FormDialog>
  );
}
