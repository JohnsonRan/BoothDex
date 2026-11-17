"use client";
// Opens the check that lifts the stricter rate limits when the API offers it (lib/api.ts, `passCheck`). Only this
// host loads with the page; the dialog, its strings and the Turnstile script load with the first offer.

import dynamic from "next/dynamic";
import { useLayoutEffect, useState } from "react";
import { passCheck } from "@/lib/api";
import type { PassAsk } from "./pass-dialog";

const PassDialog = dynamic(
  () => import("@/components/frame/sign-in").then((m) => m.PassDialog),
  { ssr: false },
);

export function PassHost() {
  const [ask, setAsk] = useState<PassAsk | null>(null);
  // a layout effect: set before any component's effect can offer the check, as a refused server read does on load
  useLayoutEffect(() => {
    let open: Promise<boolean> | undefined;
    // one dialog for every request refused while it is open; each is sent again once it passes
    passCheck.open = (siteKey) =>
      (open ??= new Promise((resolve) =>
        setAsk({
          siteKey,
          done: (passed) => {
            open = undefined;
            setAsk(null);
            resolve(passed);
          },
        }),
      ));
  }, []);
  return ask && <PassDialog {...ask} />;
}
