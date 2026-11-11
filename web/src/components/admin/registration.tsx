"use client";
// Open or close sign-up: one Switch, saved as soon as it is flipped (PUT /api/admin/settings/registration).

import { Switch } from "@astryxdesign/core/Switch";
import { useEffect, useState } from "react";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import { errorText, useAdminT } from "./messages";

export function Registration() {
  const t = useAdminT();
  const toast = useToast();
  const [open, setOpen] = useState<boolean>();
  const [busy, setBusy] = useState(false);

  // biome-ignore lint/correctness/useExhaustiveDependencies: t only words a failure
  useEffect(() => {
    api<{ registration: { open: boolean } }>("/api/admin/settings").then(
      (s) => setOpen(s.registration.open),
      (e) => toast({ type: "error", body: errorText(t, e) }),
    );
  }, [toast]);

  const change = async (next: boolean) => {
    setBusy(true);
    try {
      await api("/api/admin/settings/registration", {
        method: "PUT",
        json: { open: next },
      });
      setOpen(next);
      toast({ body: t(next ? "registration.opened" : "registration.closed") });
    } catch (e) {
      toast({ type: "error", body: errorText(t, e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Switch
      label={t("registration.open")}
      value={open ?? false}
      isDisabled={open === undefined || busy}
      onChange={change}
    />
  );
}
