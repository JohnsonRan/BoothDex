"use client";
// The Turnstile widget's place in a form: the element the widget renders into, or, when its script did not load,
// an error Banner with Try again.

import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { VStack } from "@astryxdesign/core/Layout";
import type { TurnstileState } from "@/lib/turnstile";
import { useVerificationT } from "./messages";

export function VerificationField({
  verification,
}: {
  verification: TurnstileState;
}) {
  const t = useVerificationT();
  return verification.failed ? (
    <Banner
      status="error"
      title={t("failedLoad")}
      endContent={
        <Button
          label={t("retry")}
          variant="secondary"
          onClick={verification.retry}
        />
      }
    />
  ) : (
    <VStack ref={verification.ref} className="turnstile-host" />
  );
}
