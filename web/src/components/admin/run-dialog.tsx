"use client";
// "Run with options": the Astryx block Dialog — Form (@astryxdesign/cli assets/templates/blocks/components/Dialog/DialogFormDialog.tsx):
// a DialogHeader with a subtitle, the fields in LayoutContent, Cancel and the primary action in LayoutFooter.
// Fields are only the kind's safe options (types.ts RUN_PARAMS): flags as CheckboxInput, counts as NumberInput
// with the worker's default as the placeholder; an empty field is left out of the job's params.

import { Button } from "@astryxdesign/core/Button";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import {
  HStack,
  Layout,
  LayoutContent,
  LayoutFooter,
  VStack,
} from "@astryxdesign/core/Layout";
import { NumberInput } from "@astryxdesign/core/NumberInput";
import { useState } from "react";
import { useAdminT } from "./messages";
import { type Kind, RUN_PARAMS } from "./types";

export function RunDialog({
  kind,
  onClose,
  onRun,
}: {
  kind: Kind;
  onClose: () => void;
  onRun: (params: Record<string, number | boolean>) => Promise<boolean>;
}) {
  const t = useAdminT();
  const [params, setParams] = useState<Record<string, number | boolean>>({});
  const [busy, setBusy] = useState(false);
  // fields whose typed text the NumberInput rejected: the text stays on screen while `params` keeps the last
  // accepted value, so Queue stays disabled until each is fixed or cleared. The field reports at blur, which a
  // press on Queue (or a Tab to it) causes first, so the button is disabled before that press lands.
  const [rejected, setRejected] = useState<ReadonlySet<string>>(new Set());
  const reject = (name: string, isRejected: boolean) =>
    setRejected((prev) => {
      if (prev.has(name) === isRejected) return prev;
      const next = new Set(prev);
      if (isRejected) next.add(name);
      else next.delete(name);
      return next;
    });
  const set = (name: string, v: number | boolean | null | undefined) =>
    setParams(({ [name]: _, ...rest }) =>
      v == null || v === false ? rest : { ...rest, [name]: v },
    );
  const submit = async () => {
    setBusy(true);
    if (await onRun(params)) onClose();
    setBusy(false);
  };
  return (
    <Dialog isOpen onOpenChange={onClose} purpose="form" width={480}>
      <Layout
        header={
          <DialogHeader
            title={t("run.title", { kind: t(`kind.${kind}`) })}
            subtitle={t("run.desc")}
            onOpenChange={onClose}
          />
        }
        content={
          <LayoutContent>
            <VStack gap={4}>
              {RUN_PARAMS[kind]?.map((p) =>
                p.type === "flag" ? (
                  <CheckboxInput
                    key={p.name}
                    label={t(`param.${p.name}`)}
                    description={t(`param.${p.name}Desc`)}
                    value={params[p.name] === true}
                    onChange={(v) => set(p.name, v)}
                  />
                ) : (
                  <NumberInput
                    key={p.name}
                    label={t(`param.${p.name}`)}
                    placeholder={
                      p.fallback
                        ? t("param.default", { n: p.fallback })
                        : undefined
                    }
                    // counts are whole and positive; the field commits a typed value within them
                    min={1}
                    isIntegerOnly
                    value={(params[p.name] as number | undefined) ?? null}
                    onChange={(v) => set(p.name, v)}
                    onEntryErrorChange={(e) => reject(p.name, e !== null)}
                  />
                ),
              )}
            </VStack>
          </LayoutContent>
        }
        footer={
          <LayoutFooter>
            <HStack gap={2} hAlign="end">
              <Button
                label={t("common.cancel")}
                variant="secondary"
                onClick={onClose}
              />
              <Button
                label={t("run.submit")}
                variant="primary"
                isLoading={busy}
                isDisabled={rejected.size > 0}
                onClick={submit}
              />
            </HStack>
          </LayoutFooter>
        }
      />
    </Dialog>
  );
}
