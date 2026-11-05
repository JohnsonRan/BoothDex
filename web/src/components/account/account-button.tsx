"use client";

// The top bar's last actions. Signed out: Settings (a ghost IconButton with a tooltip, as the bar's other
// preference controls) and Sign in, the ghost Button of template shell-top-nav's end actions (lines 186-194).
// Signed in: the account IconButton (the user-circle glyph of template shell-side-nav's account row, line 180),
// whose menu holds Settings too. Both are in the server HTML, each in a slot that globals.css shows by the
// `data-account` the head script sets from this browser's last session (session.tsx keeps it), so a returning
// visitor sees the right one from the first paint and nothing swaps. The dialogs' code loads after the page
// (lib/lazy), not with it; the menu's at once when the last session was signed in.

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import { useUi } from "@/components/providers";
import { Lazy, lazySurface, preloadOn, preloadWhenIdle } from "@/lib/lazy";
import { SIGNED_IN } from "@/lib/session-hint";
import { useAccount } from "./session";
import { AccountTrigger } from "./trigger";

const loadMenu = () => import("./menu");
const AccountMenu = dynamic(loadMenu, {
  ssr: false,
  // the same button until the menu's code is there
  loading: () => <AccountTrigger />,
});
const SignInDialog = lazySurface(() =>
  import("@/components/frame/sign-in").then((m) => m.SignInDialog),
);
const SavedSearches = lazySurface(() =>
  import("./saved-searches").then((m) => m.default),
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
  const [saved, setSaved] = useState<Open>();
  const [settings, setSettings] = useState<Open>();
  const signedIn = !!me;
  useEffect(() => {
    if (document.documentElement.dataset.account === SIGNED_IN) loadMenu();
  }, []);
  useEffect(
    () =>
      preloadWhenIdle(
        SettingsDialog.preload,
        signedIn ? SavedSearches.preload : SignInDialog.preload,
      ),
    [signedIn],
  );

  return (
    <>
      <span data-account-slot="in">
        {me ? (
          <AccountMenu
            me={me}
            onSavedSearches={() => setSaved(true)}
            onSettings={() => setSettings(true)}
          />
        ) : (
          <AccountTrigger />
        )}
      </span>
      <span data-account-slot="out">
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
      </span>
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
      {saved !== undefined && me && (
        <Lazy surface={SavedSearches} isOpen={saved} onOpenChange={setSaved} />
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
