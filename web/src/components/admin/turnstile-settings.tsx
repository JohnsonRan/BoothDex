"use client";
// The Cloudflare Turnstile keys that open the guestbook, laid out as the AI settings (ai-settings.tsx): the site
// key, the write-only secret showing only its hint, Save and Test. A field the server environment sets is
// read-only with the same description. Test asks Cloudflare with the saved secret; a refusal stays as a Banner
// with Cloudflare's codes, since it needs a change in the form.

import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { Spinner } from "@astryxdesign/core/Spinner";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useEffect, useState } from "react";
import { useToast } from "@/components/toasts";
import { api } from "@/lib/api";
import { locked } from "./ai-settings";
import { errorText, useAdminT } from "./messages";
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

  if (!keys) return <Spinner label={t("turnstile.title")} />;
  const env = keys.from_env;
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
          isDisabled={busy || !keys.secret_set || !keys.site_key}
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
    </VStack>
  );
}
