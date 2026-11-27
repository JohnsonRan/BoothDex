"use client";
// The create and edit forms of the Filterable Table template's saved views (`AdaptiveSavedViewForm` filled as at
// page.tsx:4558-4628) with their summary (`ViewSummaryList`, :1961-2006). Loaded on first use (lib/lazy).

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import {
  MetadataList,
  MetadataListItem,
} from "@astryxdesign/core/MetadataList";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { Token } from "@astryxdesign/core/Token";
import { useEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import { type Filters, parseFilters } from "@/lib/catalog";
import "@/lib/icons-on-demand";
import {
  type ClauseNames,
  sortLabel,
  summaryTokens,
} from "@/components/catalog/filter/clause-text";
import type { Saved, useSavedSearches } from "./saved-searches";
import { AdaptiveSavedViewForm } from "./saved-view-host";

// the API keeps at most 100 characters of a name
const NAME_MAX = 100;

type Store = ReturnType<typeof useSavedSearches>;

/** "Save this search": the name starts as the page's heading. */
export function CreateSavedForm({
  isOpen,
  onOpenChange,
  isCompactSurface,
  defaultName,
  filters,
  query,
  names,
  store,
}: {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  isCompactSurface: boolean;
  /** the page's heading */
  defaultName: string;
  filters: Filters;
  /** the URL query saved (saved-searches.ts savedQuery) */
  query: string;
  names: ClauseNames;
  store: Store;
}) {
  const { t } = useUi();
  const inputRef = useRef<HTMLInputElement>(null);
  const [name, setName] = useState("");
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (isOpen) {
      setName(defaultName.slice(0, NAME_MAX));
      setChecked(false);
    }
  }, [isOpen, defaultName]);
  const problem = checked && !name.trim() ? t("account.required") : undefined;
  const create = async () => {
    if (busy) return;
    setChecked(true);
    const trimmed = name.trim();
    if (!trimmed) {
      inputRef.current?.focus();
      return;
    }
    setBusy(true);
    if (await store.create(trimmed, query)) onOpenChange(false);
    setBusy(false);
  };
  return (
    <AdaptiveSavedViewForm
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      title={t("saved.create")}
      subtitle={t("saved.desc")}
      isCompactSurface={isCompactSurface}
    >
      <form
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          void create();
        }}
      >
        <VStack gap={4}>
          <TextInput
            ref={inputRef}
            label={t("saved.name")}
            htmlName="saved_name"
            isRequired
            enterKeyHint="done"
            value={name}
            onChange={(v) => setName(v.slice(0, NAME_MAX))}
            status={problem ? { type: "error", message: problem } : undefined}
            hasAutoFocus
          />
          <VStack gap={1}>
            <Text type="label">{t("saved.saves")}</Text>
            <SavedSummary filters={filters} names={names} />
          </VStack>
          <HStack hAlign="end">
            <Button
              type="submit"
              label={t("saved.save")}
              variant="primary"
              isLoading={busy}
            />
          </HStack>
        </VStack>
      </form>
    </AdaptiveSavedViewForm>
  );
}

/** Rename, follow by RSS, or delete a saved search; its conditions are shown, not edited. */
export function EditSavedForm({
  saved,
  onClose,
  isCompactSurface,
  names,
  store,
}: {
  /** the search being edited; null closes the form */
  saved: Saved | null;
  onClose: () => void;
  isCompactSurface: boolean;
  names: ClauseNames;
  store: Store;
}) {
  const { t } = useUi();
  const inputRef = useRef<HTMLInputElement>(null);
  const [name, setName] = useState("");
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (saved) {
      setName(saved.name);
      setChecked(false);
    }
  }, [saved]);
  const done = (ok: boolean) => ok && onClose();
  const problem = checked && !name.trim() ? t("account.required") : undefined;
  const save = async () => {
    if (busy || !saved) return;
    setChecked(true);
    const trimmed = name.trim();
    if (!trimmed) {
      inputRef.current?.focus();
      return;
    }
    if (trimmed === saved.name) return onClose();
    setBusy(true);
    const ok = await store.rename(saved, trimmed);
    setBusy(false);
    done(ok);
  };
  return (
    <AdaptiveSavedViewForm
      isOpen={saved != null}
      onOpenChange={(open) => !open && onClose()}
      title={t("saved.edit")}
      subtitle={t("saved.editDesc")}
      isCompactSurface={isCompactSurface}
    >
      <form
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <VStack gap={4}>
          <TextInput
            ref={inputRef}
            label={t("saved.name")}
            htmlName="saved_name"
            isRequired
            enterKeyHint="done"
            value={name}
            onChange={(v) => setName(v.slice(0, NAME_MAX))}
            status={problem ? { type: "error", message: problem } : undefined}
          />
          <VStack gap={1}>
            <Text type="label">{t("saved.saves")}</Text>
            {saved && (
              <SavedSummary filters={parseFilters(saved.query)} names={names} />
            )}
          </VStack>
          <HStack gap={2} vAlign="center">
            <Button
              type="button"
              label={t("common.delete")}
              variant="destructive"
              onClick={async () => saved && done(await store.remove(saved))}
            />
            <StackItem size="fill">
              <HStack gap={2} hAlign="end">
                <Button
                  type="button"
                  label={t("saved.copy")}
                  variant="secondary"
                  icon={<Icon icon="site:feed" size="sm" />}
                  onClick={() => saved && store.copyFeed(saved)}
                />
                <Button
                  type="submit"
                  label={t("saved.save")}
                  variant="primary"
                  isLoading={busy}
                />
              </HStack>
            </StackItem>
          </HStack>
        </VStack>
      </form>
    </AdaptiveSavedViewForm>
  );
}

/** ViewSummaryList: the conditions as tokens, then the sort. */
function SavedSummary({
  filters,
  names,
}: {
  filters: Filters;
  names: ClauseNames;
}) {
  const { t } = useUi();
  const tokens = summaryTokens(filters, names, t);
  return (
    <MetadataList columns="single" label={{ position: "start", width: 104 }}>
      <MetadataListItem label={t("nav.filters")}>
        {tokens.length === 0 ? (
          <Text type="body">{t("saved.none")}</Text>
        ) : (
          // tokenWrap + filterToken (:1442-1451): a long clause wraps and ellipsises inside the dialog
          <HStack gap={1} wrap="wrap">
            {tokens.map((label) => (
              <Token
                key={label}
                label={label}
                size="sm"
                style={{ minWidth: 0, maxWidth: "100%" }}
              />
            ))}
          </HStack>
        )}
      </MetadataListItem>
      <MetadataListItem label={t("sort.label")}>
        <Text type="body">{sortLabel(filters, t)}</Text>
      </MetadataListItem>
    </MetadataList>
  );
}
