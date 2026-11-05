"use client";

import { useCallback, useRef, useState } from "react";
import { toast } from "sonner";
import { ApiError, api } from "@/lib/api";
import type { Item } from "@/lib/catalog";
import { useSettings } from "@/lib/settings";
import {
  addViewed,
  loadViewed,
  recordLocalView,
  saveViewed,
} from "@/lib/viewed";
import type { Me } from "./account-menu";

/** The visitor's session: who they are, their favourites and what they viewed (in the account, else this browser). */
export function useAccount() {
  const { t } = useSettings();
  // read inside stable callbacks so switching language does not recreate them
  const tRef = useRef(t);
  tRef.current = t;

  const [me, setMe] = useState<Me | null>(null);
  const [favs, setFavs] = useState<Set<number> | null>(null);
  // opened items, newest first (null until the account or this browser's list is read)
  const [viewed, setViewed] = useState<number[] | null>(null);
  const [viewedGen, setViewedGen] = useState(0);

  // latest account load; answers to older ones are dropped
  const accountLoad = useRef(0);

  // session: who am I, my favourites, what I viewed; rerun after login/logout
  const loadAccount = useCallback(async () => {
    const gen = ++accountLoad.current;
    setViewed(null);
    const user = await api<Me>("/api/account").catch(() => null);
    // a newer sign-in/out supersedes this load; don't publish another account's data
    if (gen !== accountLoad.current) return user;
    setMe(user);
    if (user) {
      // what this browser viewed while signed out moves into the account
      const local = loadViewed();
      if (local.length)
        await api("/api/account/views", {
          method: "POST",
          json: { views: local.map(([id, at]) => ({ id, at })) },
        })
          .then(() => saveViewed([]))
          .catch(() => {});
      if (gen !== accountLoad.current) return user;
      const res = await api<{ ids: number[] }>("/api/account/views").catch(
        () => ({
          ids: [],
        }),
      );
      if (gen !== accountLoad.current) return user;
      setViewed((cur) => [
        ...(cur ?? []),
        ...res.ids.filter((id) => !cur?.includes(id)),
      ]);
    } else {
      setViewed(loadViewed().map(([id]) => id));
    }
    setViewedGen((g) => g + 1);
    const favRes = user
      ? await api<{ ids: number[] }>("/api/account/favourites").catch(() => ({
          ids: [],
        }))
      : null;
    if (gen !== accountLoad.current) return user;
    setFavs(favRes ? new Set(favRes.ids) : null);
    return user;
  }, []);

  const toggleFav = useCallback(
    async (it: Item) => {
      if (!favs) return toast(tRef.current("fav.needSignIn"));
      const on = favs.has(it.id);
      setFavs((s) => {
        const n = new Set(s);
        on ? n.delete(it.id) : n.add(it.id);
        return n;
      });
      try {
        await api(`/api/account/favourites/${it.id}`, {
          method: on ? "DELETE" : "PUT",
        });
      } catch (e) {
        setFavs((s) => {
          const n = new Set(s);
          on ? n.add(it.id) : n.delete(it.id);
          return n;
        });
        toast.error(
          e instanceof ApiError && e.status === 401
            ? tRef.current("fav.needSignIn")
            : tRef.current("fav.failed"),
        );
      }
    },
    [favs],
  );

  // remember what was opened: in the account when signed in, otherwise in this browser
  const recordView = useCallback(
    (id: number) => {
      setViewed((v) => addViewed(v ?? [], id));
      if (me)
        api("/api/account/views", {
          method: "POST",
          json: { views: [{ id }] },
        }).catch(() => {});
      else recordLocalView(id);
    },
    [me],
  );

  const clearViewed = async () => {
    try {
      if (me) await api("/api/account/views", { method: "DELETE" });
      else saveViewed([]);
      setViewed([]);
    } catch {
      toast.error(tRef.current("recent.clearFailed"));
    }
  };

  return {
    me,
    favs,
    viewed,
    viewedGen,
    loadAccount,
    toggleFav,
    recordView,
    clearViewed,
  };
}
