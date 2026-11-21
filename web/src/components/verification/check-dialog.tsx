"use client";
// A Turnstile check in a dialog, from the Astryx dialog block DialogWithSubtitle: why it is asked, the widget and a way
// out. A token is sent as soon as the widget gives one, as a reCAPTCHA checkbox does, and a taken token closes the
// dialog. After a failed send nothing more is sent by itself: the managed widget hands out a new token on its own, so
// the failure stays with Try again, which waits out a 429's Retry-After. The stricter limits' check (pass-dialog.tsx)
// and the console's check of new Turnstile keys (admin/turnstile-settings.tsx) are this dialog.

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
import { VerificationField } from "./verification-field";

/** Why a sent token was not taken, and how many seconds Try again waits. */
export type CheckFailure = { message: string; retryAfter?: number };

export function CheckDialog({
  siteKey,
  action,
  title,
  subtitle,
  retry,
  cancel,
  send,
  done,
}: {
  siteKey: string;
  /** the action the API checks the token for */
  action: string;
  title: string;
  subtitle: string;
  /** Try again's label */
  retry: string;
  /** the way out's label */
  cancel: string;
  /** sends the widget's token: null once it was taken, else why not */
  send: (token: string) => Promise<CheckFailure | null>;
  done: (passed: boolean) => void;
}) {
  const verification = useTurnstile(siteKey, action);
  const [failure, setFailure] = useState<string | null>(null);
  // Try again stays disabled while a 429's Retry-After runs
  const [cooling, setCooling] = useState(false);
  const [busy, setBusy] = useState(false);
  const submit = useEffectEvent(async (token: string) => {
    setBusy(true);
    const failed = await send(token);
    if (!failed) return done(true);
    setFailure(failed.message);
    if (failed.retryAfter) {
      setCooling(true);
      setTimeout(() => setCooling(false), failed.retryAfter * 1000);
    }
    verification.reset();
    setBusy(false);
  });
  const { token } = verification;
  useEffect(() => {
    if (token && !failure) submit(token);
  }, [token, failure]);
  const close = (open: boolean) => !open && !busy && done(false);

  return (
    <Dialog isOpen onOpenChange={close} purpose="form" width={440}>
      <Layout
        header={
          <DialogHeader
            title={title}
            subtitle={subtitle}
            onOpenChange={close}
          />
        }
        content={
          <LayoutContent>
            <VStack gap={4}>
              {failure && (
                <Banner
                  status="error"
                  title={failure}
                  endContent={
                    <Button
                      label={retry}
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
                label={cancel}
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
