"use client";
// The favourite buttons' one handler. Accounts come with the sign-in step of the rewrite; until then every
// visitor is signed out, and a click explains why nothing was saved (master's toast, as in block
// useToastHookUsage). One document listener serves every tile, so the tiles stay server markup, and the toast's
// code loads with the first click.

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";

const Toast = dynamic(() => import("./fav-toaster"), { ssr: false });

export function FavToast() {
  // clicks so far: each one raises the toast again (one toast however often it is clicked)
  const [clicks, setClicks] = useState(0);
  useEffect(() => {
    const click = (e: MouseEvent) => {
      // the buttons carry data-fav (item-tile.tsx)
      if (e.target instanceof Element && e.target.closest("[data-fav]"))
        setClicks((n) => n + 1);
    };
    document.addEventListener("click", click);
    return () => document.removeEventListener("click", click);
  }, []);
  return clicks > 0 && <Toast clicks={clicks} />;
}
