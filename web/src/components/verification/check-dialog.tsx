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
import { useEffect, useEffectEvent, useRef, useState } from "react";
import { useTurnstile } from "@/lib/turnstile";
import { VerificationField } from "./verification-field";

/** Why a sent token was not taken, and how many seconds Try again waits. */
export type CheckFailure = { message: string; retryAfter?: number };

/** What clicking Try again must ask the widget to do when it has no token ready. */
export function retryAction(widget: {
  token: string;
  failed: boolean;
}): "retry" | "reset" | "none" {
  if (widget.failed) return "retry";
  if (!widget.token) return "reset";
  return "none";
}

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
  const inFlight = useRef(false);
  const currentSiteKey = useRef(siteKey);
  currentSiteKey.current = siteKey;
  const coolTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );
  useEffect(() => () => clearTimeout(coolTimer.current), []);
  // a new siteKey (e.g. rotated while the dialog is open) drops any earlier failure without unmounting the modal
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset state whenever the target siteKey changes
  useEffect(() => {
    setFailure(null);
    setCooling(false);
    clearTimeout(coolTimer.current);
  }, [siteKey]);
  const submit = useEffectEvent(async (token: string) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    const keyAtStart = currentSiteKey.current;
    try {
      const failed = await send(token);
      if (!failed) return done(true);
      if (keyAtStart !== currentSiteKey.current) return;
      setFailure(failed.message);
      const wait = failed.retryAfter ?? 0;
      if (wait > 0) {
        setCooling(true);
        clearTimeout(coolTimer.current);
        coolTimer.current = setTimeout(() => setCooling(false), wait * 1000);
      }
      verification.reset();
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  });
  const { token } = verification;
  useEffect(() => {
    if (token && !failure) submit(token);
  }, [token, failure]);
  const close = (open: boolean) => !open && !busy && done(false);
  const onRetry = () => {
    setFailure(null);
    const act = retryAction(verification);
    if (act === "retry") verification.retry();
    else if (act === "reset") verification.reset();
  };

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
                      onClick={onRetry}
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
