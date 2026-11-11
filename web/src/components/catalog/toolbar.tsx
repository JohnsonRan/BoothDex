"use client";
// The toolbar row under the title, as the table-filter template's header (page.tsx:4373-4470): the filter row, or
// the saved-views bar while that is toggled on, then one end cluster — the sort as the view control (:3999-4006,
// a ghost `sm` menu), "Save this search" (bookmark-plus, :4439-4446), the saved-views toggle (:4450-4468) and,
// over the avatar strip, its toggle (strip-toggle.tsx).
// Saved searches belong to the account: signed out, both open the sign-in dialog. They are not offered in the
// favourites view, whose list is the account's own.

import { Button } from "@astryxdesign/core/Button";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { HStack, StackItem } from "@astryxdesign/core/Stack";
import { ToggleButton } from "@astryxdesign/core/ToggleButton";
import dynamic from "next/dynamic";
import { type ReactNode, useEffect, useState } from "react";
import { useAccount } from "@/components/account/session";
import { useUi } from "@/components/providers";
import { FILTER_RESET, type Filters, parseFilters } from "@/lib/catalog";
import { lateMenuButton } from "@/lib/late-menu";
import { preloadOn, preloadWhenIdle } from "@/lib/lazy";
import type { Translate } from "@/lib/settings-core";
import { type ClauseNames, sortLabel } from "./clause-text";
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
import { StripToggle } from "./strip-toggle";

/** The sort's button, as DropdownMenu draws it: the current sort and a chevron (sort-menu.tsx). */
export const sortButton = (filters: Filters, t: Translate) =>
  ({
    label: sortLabel(filters, t),
    variant: "ghost",
    size: CONTROL_SIZE,
    endContent: <Icon icon="chevronDown" size="sm" color="inherit" />,
  }) as const;

function SortButton() {
  const { t } = useUi();
  const { filters } = useFilterUrl();
  return <Button {...sortButton(filters, t)} {...lateMenuButton("sort")} />;
}

// the sort menu's code loads after the page, as the top bar's menus (frame/header-controls.tsx); its button keeps
// a press for it
const SortMenu = dynamic(() => import("./sort-menu").then((m) => m.SortMenu), {
  ssr: false,
  loading: () => <SortButton />,
});
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
            <SortMenu filters={filters} apply={apply} />
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
        <StripToggle />
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
