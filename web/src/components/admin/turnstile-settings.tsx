"use client";
// The Cloudflare Turnstile keys that open the guestbook, laid out as the AI settings (ai-settings.tsx): the site
// key, the write-only secret showing only its hint, Save and Test. A field the server environment sets is
// read-only with the same description. Test asks Cloudflare with the saved secret; a refusal stays as a Banner
// with Cloudflare's codes, since it needs a change in the form. Under them, a Switch saved as soon as it is flipped
// (as registration.tsx) makes sign-in and registration carry the widget, and another gives visitors without a pass
// the stricter rate limits (crates/api/src/pass.rs); while either key is unset the API checks nothing, so an off
// switch is disabled, saying why. An on switch stays operable: otherwise it would take effect, unasked, with the next
// key saved, and a mistyped site key would lock sign-in.
// A save that would put keys in front of sign-in that no check has proved is refused with `verification_required`
// (crates/api/src/admin/turnstile.rs): the check dialog shows the widget for the site key the refusal names, and the
// same save is sent again with its token, so keys that do not work together never lock anyone out.

import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Switch } from "@astryxdesign/core/Switch";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useEffect, useState } from "react";
import { useToast } from "@/components/toasts";
import {
  CheckDialog,
  type CheckFailure,
} from "@/components/verification/check-dialog";
import { ApiError, api } from "@/lib/api";
import { locked } from "./ai-settings";
import { type AdminKey, errorText, useAdminT } from "./messages";
import type { Turnstile } from "./types";

/** the action the API checks the console's token for (crates/api/src/admin/turnstile.rs SETTINGS_ACTION) */
const ACTION = "settings";

type Change = Partial<Turnstile> & { secret?: string };
/** A save waiting for a check: what it sends, the site key to prove, and what to do with the saved settings. */
type Proof = {
  change: Change;
  siteKey: string;
  saved: (k: Turnstile) => void;
};

const put = (json: object) =>
  api<Turnstile>("/api/admin/settings/turnstile", { method: "PUT", json });

export function TurnstileSettings() {
  const t = useAdminT();
  const toast = useToast();
  const [keys, setKeys] = useState<Turnstile>();
  const [siteKey, setSiteKey] = useState("");
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string[] | null>(null);
  const [proof, setProof] = useState<Proof>();

  const take = (k: Turnstile) => {
    setKeys(k);
    setSiteKey(k.site_key ?? "");
  };
  // biome-ignore lint/correctness/useExhaustiveDependencies: t only words a failure; a language change must not read the settings again over unsaved edits
  useEffect(() => {
    api<{ turnstile: Turnstile }>("/api/admin/settings").then(
      ({ turnstile }) => take(turnstile),
      (e) => toast({ type: "error", body: errorText(t, e) }),
    );
  }, [toast]);

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      toast({ type: "error", body: errorText(t, e) });
    } finally {
      setBusy(false);
    }
  };
  /** Sends `change`; one the API asks a check for waits in the check dialog, and `saved` runs once it is in. */
  const send = (change: Change, saved: (k: Turnstile) => void) =>
    run(async () => {
      try {
        saved(await put(change));
      } catch (e) {
        if (!(e instanceof ApiError && e.code === "verification_required"))
          throw e;
        setProof({ change, siteKey: e.siteKey ?? "", saved });
      }
    });
  const prove = async (
    { change, siteKey, saved }: Proof,
    token: string,
  ): Promise<CheckFailure | null> => {
    try {
      saved(await put({ ...change, token, proved_site_key: siteKey }));
      return null;
    } catch (e) {
      // another save changed the key meanwhile: the dialog, keyed by the site key, starts over with the key the API
      // names now, and this one's failure goes with it
      if (e instanceof ApiError && e.code === "verification_required") {
        setProof({ change, siteKey: e.siteKey ?? "", saved });
        return { message: t("turnstile.proofRefused") };
      }
      if (e instanceof ApiError && e.code === "verification_failed")
        return { message: t("turnstile.proofRefused") };
      if (e instanceof ApiError && e.code === "provider_failed")
        return { message: t("turnstile.proofUnjudged") };
      return { message: errorText(t, e) };
    }
  };
  const save = () =>
    send(
      // a key the environment sets is not sent, so the database does not keep a copy that would show once the
      // environment variable goes
      keys?.from_env.site_key ? { secret } : { site_key: siteKey, secret },
      (k) => {
        take(k);
        setSecret("");
        setRefused(null);
        toast({ body: t("ai.saved") });
      },
    );
  const test = () =>
    run(async () => {
      const r = await api<{ ok: boolean; codes: string[] }>(
        "/api/admin/settings/turnstile/test",
        { method: "POST" },
      );
      setRefused(r.ok ? null : r.codes);
      if (r.ok) toast({ body: t("turnstile.testOk") });
    });
  // only the switch is taken from the answer, so an unsaved edit of the site key stays in its field
  const flip = (change: Partial<Turnstile>, done: AdminKey) =>
    send(change, (k) => {
      setKeys(k);
      toast({ body: t(done) });
    });

  if (!keys) return <Spinner label={t("turnstile.title")} />;
  const env = keys.from_env;
  const keysSet = keys.secret_set && !!keys.site_key;
  return (
    <VStack gap={4}>
      <TextInput
        label={t("turnstile.siteKey")}
        description={env.site_key ? t("ai.env") : t("turnstile.siteKeyDesc")}
        isReadOnly={env.site_key}
        {...locked(env.site_key)}
        autoComplete="off"
        value={siteKey}
        onChange={setSiteKey}
      />
      <TextInput
        label={t("turnstile.secret")}
        type="password"
        autoComplete="off"
        description={
          env.secret
            ? t("ai.env")
            : keys.secret_set
              ? t("ai.keySaved", { hint: keys.secret_hint ?? "" })
              : t("ai.keyNone")
        }
        isReadOnly={env.secret}
        {...locked(env.secret)}
        value={secret}
        onChange={setSecret}
      />
      <HStack gap={2}>
        <Button
          label={t("ai.save")}
          variant="primary"
          isLoading={busy}
          onClick={save}
        />
        <Button
          label={t("turnstile.test")}
          variant="secondary"
          isDisabled={busy || !keysSet}
          onClick={test}
        />
      </HStack>
      {refused && (
        <Banner
          status="error"
          title={t("turnstile.testFailed", {
            codes: refused.join(", ") || "-",
          })}
        />
      )}
      <Switch
        label={t("turnstile.auth")}
        description={
          keysSet ? t("turnstile.authDesc") : t("error.turnstileUnset")
        }
        value={keys.auth}
        isDisabled={busy || (!keysSet && !keys.auth)}
        onChange={(auth) =>
          flip({ auth }, auth ? "turnstile.authOn" : "turnstile.authOff")
        }
      />
      <Switch
        label={t("turnstile.strict")}
        description={
          keysSet ? t("turnstile.strictDesc") : t("error.turnstileUnset")
        }
        value={keys.strict_limits}
        isDisabled={busy || (!keysSet && !keys.strict_limits)}
        onChange={(strict_limits) =>
          flip(
            { strict_limits },
            strict_limits ? "turnstile.strictOn" : "turnstile.strictOff",
          )
        }
      />
      {proof && (
        <CheckDialog
          // a key the API names anew starts the widget over
          key={proof.siteKey}
          siteKey={proof.siteKey}
          action={ACTION}
          title={t("turnstile.proofTitle")}
          subtitle={t("turnstile.proofDesc")}
          retry={t("gate.retry")}
          cancel={t("common.cancel")}
          send={(token) => prove(proof, token)}
          done={() => setProof(undefined)}
        />
      )}
    </VStack>
  );
}
