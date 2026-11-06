"use client";
// The top bar's last actions. Signed out: Settings (a ghost IconButton with a tooltip, as the bar's other
// preference controls) and Sign in, the ghost Button of template shell-top-nav's end actions (lines 186-194).
// Signed in: the account button with the display name's initial, whose menu holds Settings too. The dialogs'
// code loads after the page (lib/lazy), not with it; the menu's (with Avatar) only for a signed-in visitor.

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import { useUi } from "@/components/providers";
import { Lazy, lazySurface, preloadOn, preloadWhenIdle } from "@/lib/lazy";
import { useAccount } from "./session";

const AccountMenu = dynamic(() => import("./menu"), { ssr: false });
const SignInDialog = lazySurface(() =>
  import("@/components/frame/sign-in").then((m) => m.SignInDialog),
);
const SettingsDialog = lazySurface(() =>
  import("@/components/settings/settings-dialog").then((m) => m.default),
);

// undefined: never opened, so that dialog's code is not loaded
type Open = boolean | undefined;

export function Account() {
  const { t } = useUi();
  const { me, reload } = useAccount();
  const [signIn, setSignIn] = useState<Open>();
  const [settings, setSettings] = useState<Open>();
  const signedIn = !!me;
  useEffect(
    () =>
      preloadWhenIdle(
        SettingsDialog.preload,
        ...(signedIn ? [] : [SignInDialog.preload]),
      ),
    [signedIn],
  );

  return (
    <>
      {me ? (
        <AccountMenu me={me} onSettings={() => setSettings(true)} />
      ) : (
        <>
          <IconButton
            label={t("settings.title")}
            tooltip={t("settings.title")}
            variant="ghost"
            icon={<Icon icon="site:settings" size="sm" />}
            onClick={() => setSettings(true)}
            {...preloadOn(SettingsDialog.preload)}
          />
          <Button
            label={t("nav.signIn")}
            variant="ghost"
            onClick={() => setSignIn(true)}
            {...preloadOn(SignInDialog.preload)}
          />
        </>
      )}
      {signIn !== undefined && (
        <Lazy
          surface={SignInDialog}
          isOpen={signIn}
          onOpenChange={setSignIn}
          onSignedIn={() => {
            setSignIn(false);
            reload();
          }}
        />
      )}
      {settings !== undefined && (
        <Lazy
          surface={SettingsDialog}
          isOpen={settings}
          onOpenChange={setSettings}
        />
      )}
    </>
  );
}
