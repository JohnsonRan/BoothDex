"use client";
// The OpenAI-compatible provider for the AI jobs (master ai-settings-card.tsx). The key is write-only: the API
// returns only a hint. A field the server environment sets is read-only here. Saving a new key over a saved
// one asks first. Test connection reports success as a toast; a missing model (a warning) and a broken embedding
// model (an error) stay as Banners under the buttons, since each needs a change in the form.

import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { HStack, VStack } from "@astryxdesign/core/Layout";
import { Selector } from "@astryxdesign/core/Selector";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useToast } from "@astryxdesign/core/Toast";
import { Token } from "@astryxdesign/core/Token";
import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api";
import { type AdminKey, errorText, useAdminT } from "./messages";
import { type Ai, EFFORTS } from "./types";

type Field = "base_url" | "model" | "embed_model";
const MODEL_CHOICES = 12;

export function AiSettings() {
  const t = useAdminT();
  const toast = useToast();
  const [ai, setAi] = useState<Ai>();
  const [form, setForm] = useState<Record<Field, string>>({
    base_url: "",
    model: "",
    embed_model: "",
  });
  const [effort, setEffort] = useState("none");
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirmKey, setConfirmKey] = useState(false);
  const [models, setModels] = useState<string[]>([]);
  const [embedError, setEmbedError] = useState<string | null>(null);
  const [modelMissing, setModelMissing] = useState(false);

  const take = useCallback((a: Ai) => {
    setAi(a);
    setForm({
      base_url: a.base_url ?? "",
      model: a.model ?? "",
      embed_model: a.embed_model ?? "",
    });
    setEffort(a.reasoning_effort ?? "none");
  }, []);
  // biome-ignore lint/correctness/useExhaustiveDependencies: t only words a failure; a language change must not read the settings again over unsaved edits
  useEffect(() => {
    api<{ ai: Ai }>("/api/admin/settings").then(
      ({ ai }) => take(ai),
      (e) => toast({ type: "error", body: errorText(t, e) }),
    );
  }, [toast, take]);

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
        await api<Ai>("/api/admin/settings/ai", {
          method: "PUT",
          json: { ...form, reasoning_effort: effort, api_key: key },
        }),
      );
      setKey("");
      setConfirmKey(false);
      toast({ body: t("ai.saved") });
    });
  const test = () =>
    run(async () => {
      const r = await api<{
        models: string[];
        model_found: boolean;
        embed_error?: string | null;
      }>("/api/admin/settings/ai/test", { method: "POST" });
      setModels(r.models);
      setEmbedError(r.embed_error ?? null);
      setModelMissing(!r.model_found);
      toast({ body: t("ai.testOk", { n: r.models.length }) });
    });

  if (!ai) return <Spinner label={t("ai.title")} />;
  const env = ai.from_env ?? {};
  const input = (field: Field, label: AdminKey, desc?: AdminKey) => (
    <TextInput
      label={t(label)}
      description={env[field] ? t("ai.env") : desc && t(desc)}
      isReadOnly={env[field]}
      value={form[field]}
      onChange={(v) => setForm((f) => ({ ...f, [field]: v }))}
    />
  );
  const picks = models
    .filter((m) => m !== form.model && m.includes(form.model))
    .slice(0, MODEL_CHOICES);

  return (
    <VStack gap={4}>
      {input("base_url", "ai.baseUrl", "ai.baseUrlDesc")}
      {input("model", "ai.model")}
      {picks.length > 0 && !env.model && (
        <VStack gap={2}>
          <Text type="supporting" color="secondary">
            {t("ai.modelPick")}
          </Text>
          <HStack gap={1} wrap="wrap">
            {picks.map((m) => (
              <Token
                key={m}
                size="sm"
                label={m}
                onClick={() => setForm((f) => ({ ...f, model: m }))}
              />
            ))}
          </HStack>
        </VStack>
      )}
      <Selector
        label={t("ai.effort")}
        description={env.reasoning_effort ? t("ai.env") : t("ai.effortDesc")}
        isReadOnly={env.reasoning_effort}
        value={effort}
        options={EFFORTS.map((e) => ({ value: e, label: t(`effort.${e}`) }))}
        onChange={setEffort}
      />
      {input("embed_model", "ai.embed", "ai.embedDesc")}
      <TextInput
        label={t("ai.key")}
        type="password"
        autoComplete="off"
        description={
          env.api_key
            ? t("ai.env")
            : ai.api_key_set
              ? t("ai.keySaved", { hint: ai.api_key_hint ?? "" })
              : t("ai.keyNone")
        }
        isReadOnly={env.api_key}
        value={key}
        onChange={setKey}
      />
      <HStack gap={2}>
        <Button
          label={t("ai.save")}
          variant="primary"
          isLoading={busy}
          onClick={() =>
            key.trim() && ai.api_key_set ? setConfirmKey(true) : save()
          }
        />
        <Button
          label={t("ai.test")}
          variant="secondary"
          isDisabled={busy || !ai.api_key_set}
          onClick={test}
        />
      </HStack>
      {modelMissing && <Banner status="warning" title={t("ai.testNoModel")} />}
      {embedError && (
        <Banner
          status="error"
          title={t("ai.testEmbed", { error: embedError })}
        />
      )}
      <AlertDialog
        isOpen={confirmKey}
        onOpenChange={setConfirmKey}
        title={t("key.title")}
        description={t("key.desc")}
        actionLabel={t("key.action")}
        actionVariant="destructive"
        isActionLoading={busy}
        onAction={save}
      />
    </VStack>
  );
}
