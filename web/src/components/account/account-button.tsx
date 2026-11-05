"use client";
// The top bar's last actions. Signed out: Settings (a ghost IconButton with a tooltip, as the bar's other
// preference controls) and Sign in, the ghost Button of template shell-top-nav's end actions (lines 186-194).
// Signed in: the account button with the display name's initial, whose menu holds Settings too. Each dialog's
// code loads on its first open; the menu's (with Avatar) only for a signed-in visitor.

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import dynamic from "next/dynamic";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { useAccount } from "./session";

const AccountMenu = dynamic(() => import("./menu"), { ssr: false });
const SignInDialog = dynamic(
  () => import("@/components/frame/sign-in").then((m) => m.SignInDialog),
  { ssr: false },
);
const SavedSearches = dynamic(() => import("./saved-searches"), {
  ssr: false,
});
const SettingsDialog = dynamic(
  () => import("@/components/settings/settings-dialog"),
  { ssr: false },
);

// undefined: never opened, so that dialog's code is not loaded
type Open = boolean | undefined;

export function Account() {
  const { t } = useUi();
  const { me, reload } = useAccount();
  const [signIn, setSignIn] = useState<Open>();
  const [saved, setSaved] = useState<Open>();
  const [settings, setSettings] = useState<Open>();

  return (
    <>
      {me ? (
        <AccountMenu
          me={me}
          onSavedSearches={() => setSaved(true)}
          onSettings={() => setSettings(true)}
        />
      ) : (
        <>
          <IconButton
            label={t("settings.title")}
            tooltip={t("settings.title")}
            variant="ghost"
            icon={<Icon icon="site:settings" size="sm" />}
            onClick={() => setSettings(true)}
          />
          <Button
            label={t("nav.signIn")}
            variant="ghost"
            onClick={() => setSignIn(true)}
          />
        </>
      )}
      {signIn !== undefined && (
        <SignInDialog
          isOpen={signIn}
          onOpenChange={setSignIn}
          onSignedIn={() => {
            setSignIn(false);
            reload();
          }}
        />
      )}
      {saved !== undefined && me && (
        <SavedSearches isOpen={saved} onOpenChange={setSaved} />
      )}
      {settings !== undefined && (
        <SettingsDialog isOpen={settings} onOpenChange={setSettings} />
      )}
    </>
  );
}
