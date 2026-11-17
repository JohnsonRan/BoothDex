"use client";
// The check that lifts the stricter rate limits (crates/api/src/pass.rs), from the Astryx dialog block
// DialogWithSubtitle: why it is asked, the Turnstile widget and Not now. A token is sent as soon as the widget gives
// one, as a reCAPTCHA checkbox does; a pass closes the dialog and the refused request is sent again (lib/api.ts).
// After a failed send nothing more is sent by itself: the managed widget hands out a new token on its own, so the
// failure stays with Try again, which waits out a 429's Retry-After. Loaded by pass-host.tsx with the first offer.

import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import {
  HStack,
  Layout,
  LayoutContent,
  LayoutFooter,
  VStack,
} from "@astryxdesign/core/Layout";
import { useEffect, useEffectEvent, useState } from "react";
import { useTurnstile } from "@/lib/turnstile";
import { useVerificationT, type VerificationKey } from "./messages";
import { VerificationField } from "./verification-field";

/** the action the API checks the token for (crates/api/src/pass.rs ACTION) */
const ACTION = "pass";

export type PassAsk = { siteKey: string; done: (passed: boolean) => void };

export default function PassDialog({ siteKey, done }: PassAsk) {
  const t = useVerificationT();
  const verification = useTurnstile(siteKey, ACTION);
  const [failure, setFailure] = useState<VerificationKey | null>(null);
  // Try again stays disabled while a 429's Retry-After runs
  const [cooling, setCooling] = useState(false);
  const [busy, setBusy] = useState(false);
  const send = useEffectEvent(async (token: string) => {
    setBusy(true);
    // fetched here, not through api(): a refusal's Retry-After is needed, and a 429 here offers nothing
    const res = await fetch("/api/verify", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ token }),
    }).catch(() => null);
    if (res?.ok) return done(true);
    const body = await res?.json().catch(() => null);
    // a token Cloudflare refused; otherwise the check could not be made, or this address asked too often
    setFailure(
      body?.code === "verification_failed" ? "refused" : "unavailable",
    );
    const wait =
      res?.status === 429 ? Number(res.headers.get("retry-after")) : 0;
    if (wait > 0) {
      setCooling(true);
      setTimeout(() => setCooling(false), wait * 1000);
    }
    verification.reset();
    setBusy(false);
  });
  const { token } = verification;
  useEffect(() => {
    if (token && !failure) send(token);
  }, [token, failure]);
  const close = (open: boolean) => !open && !busy && done(false);

  return (
    <Dialog isOpen onOpenChange={close} purpose="form" width={440}>
      <Layout
        header={
          <DialogHeader
            title={t("passTitle")}
            subtitle={t("passDesc")}
            onOpenChange={close}
          />
        }
        content={
          <LayoutContent>
            <VStack gap={4}>
              {failure && (
                <Banner
                  status="error"
                  title={t(failure)}
                  endContent={
                    <Button
                      label={t("retry")}
                      variant="secondary"
                      isDisabled={cooling}
                      onClick={() => setFailure(null)}
                    />
                  }
                />
              )}
              <VerificationField verification={verification} />
            </VStack>
          </LayoutContent>
        }
        footer={
          <LayoutFooter>
            <HStack hAlign="end">
              <Button
                label={t("notNow")}
                variant="secondary"
                isDisabled={busy}
                onClick={() => done(false)}
              />
            </HStack>
          </LayoutFooter>
        }
      />
    </Dialog>
  );
}
