"use client";
// The toolbar row under the title, as the table-filter template's header (page.tsx:4373-4470): the filter row, or
// the saved-views bar while that is toggled on, then one end cluster — the sort as the view control (:3999-4006,
// a ghost `sm` menu), "Save this search" (bookmark-plus, :4439-4446) and the saved-views toggle (:4450-4468).
// Saved searches belong to the account: signed out, both open the sign-in dialog. They are not offered in the
// favourites view, whose list is the account's own.

import {
  DropdownMenu,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from "@astryxdesign/core/DropdownMenu";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { HStack, StackItem } from "@astryxdesign/core/Stack";
import { ToggleButton } from "@astryxdesign/core/ToggleButton";
import dynamic from "next/dynamic";
import { type ReactNode, useEffect, useState } from "react";
import { useAccount } from "@/components/account/session";
import { useUi } from "@/components/providers";
import { FILTER_RESET, parseFilters } from "@/lib/catalog";
import { preloadOn, preloadWhenIdle } from "@/lib/lazy";
import { type ClauseNames, SORTS, sortKey, sortLabel } from "./clause-text";
import { FilterBar } from "./filter-bar";
import {
  COMPACT_SURFACE_QUERY,
  CONTROL_SIZE,
  type Facets,
} from "./filter-style";
import { useFilterUrl } from "./filter-url";
import {
  isSavable,
  type Saved,
  savedQuery,
  useSavedSearches,
} from "./saved-searches";

const loadBar = () => import("./saved-views");
const loadForms = () => import("./saved-view-forms");
const SavedViewsBar = dynamic(() => loadBar().then((m) => m.SavedViewsBar), {
  ssr: false,
});
const CreateSavedForm = dynamic(
  () => loadForms().then((m) => m.CreateSavedForm),
  { ssr: false },
);
const EditSavedForm = dynamic(() => loadForms().then((m) => m.EditSavedForm), {
  ssr: false,
});
const loadSignIn = () => import("@/components/frame/sign-in");
const SignInDialog = dynamic(() => loadSignIn().then((m) => m.SignInDialog), {
  ssr: false,
});

export function Toolbar({
  title,
  facets,
  names,
  count,
}: {
  /** the page heading, which a new saved search is named after */
  title: string;
  facets: Promise<Facets>;
  names: ClauseNames;
  /** the result count, rendered on the server */
  count: ReactNode;
}) {
  const { t } = useUi();
  const { filters, apply } = useFilterUrl();
  const { me, reload } = useAccount();
  const isCompactSurface = useMediaQuery(COMPACT_SURFACE_QUERY);
  const store = useSavedSearches(me?.id);
  const [isBarOpen, setIsBarOpen] = useState(false);
  // undefined: never opened, so that code is not loaded
  const [creating, setCreating] = useState<boolean>();
  const [editing, setEditing] = useState<Saved | null>();
  const [signIn, setSignIn] = useState<boolean>();
  // signing out or into another account closes the saved views and their forms, which were the last account's
  const [account, setAccount] = useState(me?.id);
  if (account !== me?.id) {
    setAccount(me?.id);
    setIsBarOpen(false);
    setCreating((c) => c && false);
    setEditing((e) => e && null);
  }
  const hasSaved = filters.view !== "favourites";
  const query = savedQuery(filters);
  const active = isBarOpen
    ? store.items?.find((s) => savedQuery(parseFilters(s.query)) === query)
    : undefined;
  useEffect(() => {
    if (me) preloadWhenIdle(loadBar, loadForms);
  }, [me]);

  // signed out, saving asks to sign in first
  const signedIn = (run: () => void) => () => (me ? run() : setSignIn(true));

  return (
    <HStack gap={3} vAlign="center" wrap="wrap">
      {/* toolbarPrimary (:1390-1395): takes the whole line below an 860 px header (app/filter-row.css) */}
      <StackItem size="fill" className="filter-toolbar-primary">
        {isBarOpen && hasSaved && me ? (
          <SavedViewsBar
            items={store.items}
            active={active}
            // the All chip drops the saved search's conditions; the sort stays, as the template keeps the view
            onApply={(saved) =>
              apply(
                saved
                  ? parseFilters(saved.query)
                  : { ...FILTER_RESET, q: "", author: "", avatar: "" },
              )
            }
            onEdit={setEditing}
          />
        ) : (
          <FilterBar facets={facets} names={names} count={count} />
        )}
      </StackItem>
      {/* toolbarEnd (:1398-1400): one flex item, so the cluster drops below the filter row together */}
      <HStack gap={3} vAlign="center" style={{ marginInlineStart: "auto" }}>
        {!(isBarOpen && hasSaved) && (
          <>
            {/* a radio group, as the top bar's language and currency menus (frame/header-controls.tsx) */}
            <DropdownMenu
              button={{
                label: sortLabel(filters, t),
                variant: "ghost",
                size: CONTROL_SIZE,
              }}
              alignment="end"
            >
              <DropdownMenuRadioGroup
                label={t("sort.label")}
                value={sortKey(filters)}
                onChange={(v) => {
                  const picked = SORTS.find(([, , key]) => key === v);
                  if (picked) apply({ sort: picked[0], dir: picked[1] });
                }}
              >
                {SORTS.map(([, , key]) => (
                  <DropdownMenuRadioItem
                    key={key}
                    value={key}
                    label={sortLabel(filters, t, key)}
                  />
                ))}
              </DropdownMenuRadioGroup>
            </DropdownMenu>
            {hasSaved && (
              <IconButton
                label={t("saved.create")}
                tooltip={t("saved.create")}
                variant="ghost"
                size={CONTROL_SIZE}
                icon={<Icon icon="site:save" size="sm" />}
                isDisabled={!isSavable(filters)}
                onClick={signedIn(() => setCreating(true))}
                {...preloadOn(me ? loadForms : loadSignIn)}
              />
            )}
          </>
        )}
        {hasSaved && (
          <ToggleButton
            label={t("saved.title")}
            tooltip={t("saved.title")}
            isIconOnly
            size={CONTROL_SIZE}
            isPressed={isBarOpen}
            icon={<Icon icon="site:saved" size="sm" />}
            pressedIcon={<Icon icon="site:savedFilled" size="sm" />}
            onPressedChange={(next) =>
              signedIn(() => {
                setIsBarOpen(next);
                if (next) store.load();
              })()
            }
          />
        )}
      </HStack>
      {creating !== undefined && (
        <CreateSavedForm
          isOpen={creating}
          onOpenChange={setCreating}
          isCompactSurface={isCompactSurface}
          defaultName={title}
          filters={filters}
          query={query}
          names={names}
          store={store}
        />
      )}
      {editing !== undefined && (
        <EditSavedForm
          saved={editing}
          onClose={() => setEditing(null)}
          isCompactSurface={isCompactSurface}
          names={names}
          store={store}
        />
      )}
      {signIn !== undefined && (
        <SignInDialog
          isOpen={signIn}
          onOpenChange={setSignIn}
          onSignedIn={() => {
            setSignIn(false);
            reload();
          }}
        />
      )}
    </HStack>
  );
}
