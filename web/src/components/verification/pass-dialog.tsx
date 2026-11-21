"use client";
// The check that lifts the stricter rate limits (crates/api/src/pass.rs): the check dialog (check-dialog.tsx) sending
// its token to `/api/verify`. A pass closes it and the refused request is sent again (lib/api.ts). Loaded by
// pass-host.tsx with the first offer.

import { CheckDialog, type CheckFailure } from "./check-dialog";
import { useVerificationT, type VerificationKey } from "./messages";

/** the action the API checks the token for (crates/api/src/pass.rs ACTION) */
const ACTION = "pass";

export type PassAsk = { siteKey: string; done: (passed: boolean) => void };

/** Sends the widget's token to `/api/verify`: null once the pass is set, else why not, worded by `t`. */
export async function verify(
  token: string,
  t: (key: VerificationKey) => string,
): Promise<CheckFailure | null> {
  // fetched here, not through api(): a refusal's Retry-After is needed, and a 429 here offers nothing
  const res = await fetch("/api/verify", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ token }),
  }).catch(() => null);
  if (res?.ok) return null;
  const body = await res?.json().catch(() => null);
  // a token Cloudflare refused; otherwise the check could not be made, or this address asked too often
  return {
    message: t(
      body?.code === "verification_failed" ? "refused" : "unavailable",
    ),
    retryAfter:
      res?.status === 429 ? Number(res.headers.get("retry-after")) : 0,
  };
}

export default function PassDialog({ siteKey, done }: PassAsk) {
  const t = useVerificationT();
  return (
    <CheckDialog
      siteKey={siteKey}
      action={ACTION}
      title={t("passTitle")}
      subtitle={t("passDesc")}
      retry={t("retry")}
      cancel={t("notNow")}
      send={(token) => verify(token, t)}
      done={done}
    />
  );
}
