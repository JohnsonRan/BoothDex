"use client";
// The top bar's guestbook button, a ghost IconButton with a tooltip like its neighbours (header-controls.tsx). The
// drawer's code, its strings and the Turnstile script load on the first open (pointing at or focusing the button
// starts the code), never with the page.

import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { Lazy, lazySurface, preloadOn } from "@/lib/lazy";

const GuestbookDrawer = lazySurface(() =>
  import("./guestbook-drawer").then((m) => m.GuestbookDrawer),
);

export function Guestbook() {
  const { t } = useUi();
  // undefined: never opened, so the drawer's code is not loaded
  const [open, setOpen] = useState<boolean>();
  return (
    <>
      <IconButton
        label={t("nav.guestbook")}
        tooltip={t("nav.guestbook")}
        variant="ghost"
        icon={<Icon icon="site:guestbook" size="sm" />}
        onClick={() => setOpen(true)}
        {...preloadOn(GuestbookDrawer.preload)}
      />
      {open !== undefined && (
        <Lazy surface={GuestbookDrawer} isOpen={open} onOpenChange={setOpen} />
      )}
    </>
  );
}
