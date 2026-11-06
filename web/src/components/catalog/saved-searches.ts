"use client";
// The account's saved searches (/api/account/searches): a saved search is a name and the page's URL query. Read
// when the saved-views bar opens; every change reads the list again. The list is the signed-in account's: an
// answer for an account that is no longer signed in is dropped. Done actions confirm with an info toast, failed
// ones raise an error toast.

import { useCallback, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import { useToast } from "@/components/toasts";
import { api, errorKey } from "@/lib/api";
import { DEFAULT_FILTERS, type Filters, filtersToQuery } from "@/lib/catalog";
import { copyText } from "@/lib/clipboard";
import { API_ERRORS, type MessageKey } from "@/lib/messages";

export type Saved = { id: number; name: string; query: string };

/** The URL query a search is saved and matched by: the items view of it (favourites and recent are not searches). */
export const savedQuery = (f: Filters) =>
  filtersToQuery({ ...f, view: f.view === "avatars" ? "avatars" : "items" });

/** Whether `f` narrows the catalogue at all, so there is something to save. */
export const isSavable = (f: Filters) =>
  savedQuery({ ...f, sort: DEFAULT_FILTERS.sort, dir: DEFAULT_FILTERS.dir }) !==
  "";

/** `account`: the signed-in account's id, none signed out (or before the session is read). */
export function useSavedSearches(account: number | undefined) {
  const { t } = useUi();
  const toast = useToast();
  // the list as read, with the account it was read for
  const [read, setRead] = useState<{ account: number; items: Saved[] }>();
  // null until read for this account
  const items = read && read.account === account ? read.items : null;
  const current = useRef(account);
  current.current = account;

  const fail = useCallback(
    (err: unknown, fallback: MessageKey) =>
      toast({
        type: "error",
        body: t(errorKey(err, API_ERRORS, fallback)),
        uniqueID: "saved",
      }),
    [t, toast],
  );
  const load = useCallback(async () => {
    const owner = current.current;
    if (owner === undefined) return;
    await api<{ items: Saved[] }>("/api/account/searches").then(
      (r) => {
        if (current.current === owner)
          setRead({ account: owner, items: r.items });
      },
      (err) => {
        if (current.current === owner) fail(err, "saved.loadFailed");
      },
    );
  }, [fail]);
  /** Runs one change; true when it went through. */
  const change = async (
    run: () => Promise<unknown>,
    fallback: MessageKey,
    done?: MessageKey,
  ) => {
    try {
      await run();
      if (done) toast({ body: t(done), uniqueID: "saved" });
      load();
      return true;
    } catch (err) {
      fail(err, fallback);
      return false;
    }
  };

  return {
    items,
    load,
    create: (name: string, query: string) =>
      change(
        () =>
          api("/api/account/searches", {
            method: "POST",
            json: { name, query },
          }),
        "saved.saveFailed",
        "saved.saved",
      ),
    rename: (s: Saved, name: string) =>
      change(
        () =>
          api(`/api/account/searches/${s.id}`, {
            method: "PATCH",
            json: { name },
          }),
        "saved.renameFailed",
      ),
    remove: (s: Saved) =>
      change(
        () => api(`/api/account/searches/${s.id}`, { method: "DELETE" }),
        "saved.deleteFailed",
      ),
    // feed links point at this site's origin, whatever the API was told
    copyFeed: (s: Saved) =>
      copyText(`${location.origin}/api/rss?${s.query}`).then(
        () => toast({ body: t("saved.copied"), uniqueID: "saved" }),
        (err) => fail(err, "saved.copyFailed"),
      ),
  };
}
