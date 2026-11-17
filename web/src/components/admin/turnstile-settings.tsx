"use client";
// The Cloudflare Turnstile keys that open the guestbook, laid out as the AI settings (ai-settings.tsx): the site
// key, the write-only secret showing only its hint, Save and Test. A field the server environment sets is
// read-only with the same description. Test asks Cloudflare with the saved secret; a refusal stays as a Banner
// with Cloudflare's codes, since it needs a change in the form. Under them, a Switch saved as soon as it is flipped
// (as registration.tsx) makes sign-in and registration carry the widget, and another gives visitors without a pass
// the stricter rate limits (crates/api/src/pass.rs); both are disabled, saying why, while either key is unset, since
// the API checks nothing then.

import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Switch } from "@astryxdesign/core/Switch";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useEffect, useState } from "react";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import { locked } from "./ai-settings";
import { type AdminKey, errorText, useAdminT } from "./messages";
import type { Turnstile } from "./types";

export function TurnstileSettings() {
  const t = useAdminT();
  const toast = useToast();
  const [keys, setKeys] = useState<Turnstile>();
  const [siteKey, setSiteKey] = useState("");
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string[] | null>(null);

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
  const save = () =>
    run(async () => {
      take(
        await api<Turnstile>("/api/admin/settings/turnstile", {
          method: "PUT",
          // a key the environment sets is not sent, so the database does not keep a copy that would show once the
          // environment variable goes
          json: keys?.from_env.site_key
            ? { secret }
            : { site_key: siteKey, secret },
        }),
      );
      setSecret("");
      setRefused(null);
      toast({ body: t("ai.saved") });
    });
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
    run(async () => {
      setKeys(
        await api<Turnstile>("/api/admin/settings/turnstile", {
          method: "PUT",
          json: change,
        }),
      );
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
        description={keysSet ? undefined : t("error.turnstileUnset")}
        value={keys.auth}
        isDisabled={busy || !keysSet}
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
        isDisabled={busy || !keysSet}
        onChange={(strict_limits) =>
          flip(
            { strict_limits },
            strict_limits ? "turnstile.strictOn" : "turnstile.strictOff",
          )
        }
      />
    </VStack>
  );
}
