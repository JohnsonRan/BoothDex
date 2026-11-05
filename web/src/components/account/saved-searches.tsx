"use client";
// Saved searches, master's dialog (account-menu.tsx:281-383) as block DialogFormDialog: a DialogHeader with the
// description, then the name field and "Subscribe to this search" for the current search, and the saved ones
// as List rows linking to their search, with Copy RSS and a delete confirmed in a Popover (block
// PopoverConfirmAction: a dialog is not opened from a dialog). Done actions confirm with a toast, failed ones
// raise an error toast; a list that cannot be read says so in an inline Banner.

import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Center } from "@astryxdesign/core/Center";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Icon } from "@astryxdesign/core/Icon";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { List, ListItem } from "@astryxdesign/core/List";
import { Spinner } from "@astryxdesign/core/Spinner";
import { HStack, Stack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useToast } from "@astryxdesign/core/Toast";
import { useCallback, useEffect, useState } from "react";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import { filtersToQuery } from "@/lib/catalog";
import type { MessageKey } from "@/lib/messages";
import { useDelayed } from "@/lib/use-delayed";
import { DeleteSaved } from "./delete-saved";
import "@/lib/icons-on-demand";

type Saved = { id: number; name: string; query: string };

export default function SavedSearches({
  isOpen,
  onOpenChange,
}: {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useUi();
  const toast = useToast();
  const { filters } = useFilterUrl();
  // null while loading; false when the list could not be read
  const [items, setItems] = useState<Saved[] | null | false>(null);
  const [name, setName] = useState("");
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const spinning = useDelayed(items === null);
  // the name and its button stack on a phone, where the button would lose its label
  const narrow = useMediaQuery("(max-width: 640px)");

  const load = useCallback(() => {
    api<{ items: Saved[] }>("/api/account/searches").then(
      (r) => setItems(r.items),
      () => setItems(false),
    );
  }, []);
  useEffect(() => {
    if (!isOpen) return;
    // the default name is the page's heading, as on master
    setName(document.querySelector("h1")?.textContent?.trim() ?? "");
    setChecked(false);
    load();
  }, [isOpen, load]);

  const fail = (err: unknown, fallback: MessageKey) =>
    toast({
      type: "error",
      body: err instanceof Error && err.message ? err.message : t(fallback),
      uniqueID: "saved",
    });
  const subscribe = async () => {
    setChecked(true);
    if (!name.trim()) return;
    setBusy(true);
    try {
      await api("/api/account/searches", {
        method: "POST",
        // the items view of what is on screen; the favourites and recent views are not searches
        json: {
          name: name.trim(),
          query: filtersToQuery({ ...filters, view: "items" }) || "sort=newest",
        },
      });
      toast({ body: t("saved.saved"), uniqueID: "saved" });
      load();
    } catch (err) {
      fail(err, "saved.saveFailed");
    } finally {
      setBusy(false);
    }
  };
  const remove = async (s: Saved) => {
    try {
      await api(`/api/account/searches/${s.id}`, { method: "DELETE" });
      load();
    } catch (err) {
      fail(err, "saved.deleteFailed");
    }
  };
  // feed links point at this site's origin, whatever the API was told
  const copyFeed = (s: Saved) =>
    navigator.clipboard.writeText(`${location.origin}/api/rss?${s.query}`).then(
      () => toast({ body: t("saved.copied"), uniqueID: "saved" }),
      (err) => fail(err, "saved.copyFailed"),
    );

  return (
    <Dialog
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      purpose="form"
      width={560}
    >
      <Layout
        header={
          <DialogHeader
            title={t("account.savedSearches")}
            subtitle={t("saved.desc")}
            onOpenChange={onOpenChange}
          />
        }
        content={
          <LayoutContent>
            <VStack gap={4}>
              <Stack
                direction={narrow ? "vertical" : "horizontal"}
                gap={2}
                hAlign={narrow ? "stretch" : undefined}
                vAlign={narrow ? undefined : "end"}
              >
                <StackItem size="fill">
                  <TextInput
                    label={t("saved.name")}
                    value={name}
                    // the API keeps at most 100 characters
                    onChange={(v) => setName(v.slice(0, 100))}
                    onEnter={subscribe}
                    status={
                      checked && !name.trim()
                        ? { type: "error", message: t("account.required") }
                        : undefined
                    }
                  />
                </StackItem>
                <Button
                  label={t("saved.save")}
                  variant="primary"
                  icon={<Icon icon="site:subscribe" size="sm" />}
                  isLoading={busy}
                  onClick={subscribe}
                />
              </Stack>
              {items === false ? (
                <Banner status="error" title={t("saved.loadFailed")} />
              ) : items === null ? (
                <Center>{spinning && <Spinner />}</Center>
              ) : items.length === 0 ? (
                <Text type="supporting" color="secondary">
                  {t("saved.empty")}
                </Text>
              ) : (
                <List hasDividers edge="flush">
                  {items.map((s) => (
                    <ListItem
                      key={s.id}
                      label={s.name}
                      href={`/?${s.query}`}
                      // the row's link takes its own clicks, which `onClick` does not see
                      onClickCapture={(e) => {
                        if ((e.target as Element).closest("a"))
                          onOpenChange(false);
                      }}
                      endContent={
                        <HStack gap={1}>
                          <Button
                            label={t("saved.copy")}
                            variant="secondary"
                            size="sm"
                            icon={<Icon icon="site:savedSearch" size="sm" />}
                            onClick={() => copyFeed(s)}
                          />
                          <DeleteSaved
                            name={s.name}
                            onConfirm={() => remove(s)}
                          />
                        </HStack>
                      }
                    />
                  ))}
                </List>
              )}
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}
