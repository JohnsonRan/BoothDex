"use client";
// The check that lifts the stricter rate limits (crates/api/src/pass.rs), from the Astryx dialog block
// DialogWithSubtitle: why it is asked, the Turnstile widget and Not now. A token is sent as soon as the widget gives
// one, as a reCAPTCHA checkbox does; a pass closes the dialog and the refused request is sent again (lib/api.ts).
// Loaded by pass-host.tsx with the first offer.

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
import { api } from "@/lib/api";
import { useTurnstile } from "@/lib/turnstile";
import {
  useVerificationT,
  type VerificationKey,
  verificationRefused,
} from "./messages";
import { VerificationField } from "./verification-field";

/** the action the API checks the token for (crates/api/src/pass.rs ACTION) */
const ACTION = "pass";

export type PassAsk = { siteKey: string; done: (passed: boolean) => void };

export default function PassDialog({ siteKey, done }: PassAsk) {
  const t = useVerificationT();
  const verification = useTurnstile(siteKey, ACTION);
  const [failure, setFailure] = useState<VerificationKey | null>(null);
  const [busy, setBusy] = useState(false);
  const send = useEffectEvent(async (token: string) => {
    setBusy(true);
    setFailure(null);
    try {
      await api("/api/verify", { method: "POST", json: { token } });
      done(true);
    } catch (e) {
      // a token Cloudflare refused; otherwise the check could not be made, or this address asked too often
      setFailure(verificationRefused(e) ? "refused" : "unavailable");
      verification.reset();
      setBusy(false);
    }
  });
  const { token } = verification;
  useEffect(() => {
    if (token) send(token);
  }, [token]);
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
              {failure && <Banner status="error" title={t(failure)} />}
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
