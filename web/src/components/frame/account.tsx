"use client";
// The top bar's last action: Sign in when signed out (opens SignInDialog), else the account button with the
// display name's initial. The full account menu (favourites, recent views, saved searches) is the next step of
// the rewrite; for now the menu names the account and signs out, as master's did first and last.

import { Button } from "@astryxdesign/core/Button";
import dynamic from "next/dynamic";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { useUi } from "@/components/providers";
import { fetchAccount } from "@/lib/api";

// the menu's code (with Avatar) loads only for a signed-in visitor
const AccountMenu = dynamic(() => import("./account-menu"), { ssr: false });
// the dialog's code loads on first open
const SignInDialog = dynamic(
  () => import("./sign-in").then((m) => m.SignInDialog),
  { ssr: false },
);

type Me = { display_name: string; username: string };

export function Account() {
  const { t } = useUi();
  const router = useRouter();
  const [me, setMe] = useState<Me | null>(null);
  // undefined: never opened, so the dialog's code is not loaded
  const [open, setOpen] = useState<boolean>();
  const load = useCallback((fresh?: boolean) => {
    fetchAccount<Me>({ fresh })
      .then(setMe)
      .catch(() => setMe(null));
  }, []);
  useEffect(() => load(), [load]);

  if (me)
    return (
      <AccountMenu
        name={me.display_name || me.username}
        onSignedOut={() => {
          setMe(null);
          router.refresh();
        }}
      />
    );
  return (
    <>
      <Button
        label={t("nav.signIn")}
        variant="ghost"
        onClick={() => setOpen(true)}
      />
      {open !== undefined && (
        <SignInDialog
          isOpen={open}
          onOpenChange={setOpen}
          onSignedIn={() => {
            setOpen(false);
            load(true);
            router.refresh();
          }}
        />
      )}
    </>
  );
}
