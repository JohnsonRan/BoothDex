"use client";
// The visitor's session on every route, as master's use-account.ts: who is signed in, their favourites and the
// items they opened (in the account when signed in, else in this browser). The frame provides it
// (site-frame.tsx); the account button, the favourite buttons, the viewed marks and the recent view read it.
// Notices go through one toaster that loads with the first notice, so the page does not ship the toast code.

import dynamic from "next/dynamic";
import { useRouter } from "next/navigation";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useUi } from "@/components/providers";
import { ApiError, api, fetchAccount } from "@/lib/api";
import type { MessageKey } from "@/lib/messages";
import { accountPatch, setPreferenceSync } from "@/lib/preferences";
import {
  addViewed,
  loadViewed,
  recordLocalView,
  saveViewed,
} from "@/lib/viewed";

const Toaster = dynamic(() => import("./toaster"), { ssr: false });

export type Me = {
  id: number;
  username: string;
  email: string;
  display_name: string;
  is_admin: boolean;
  preferences: Record<string, string>;
};

/** One toast: info confirms or explains, error reports a failure (and stays until dismissed). */
export type Notice = { key: MessageKey; type?: "error"; n: number };

type Session = {
  /** undefined until the first answer, then the account or null (signed out) */
  me: Me | null | undefined;
  /** the account's favourites; null signed out */
  favs: Set<number> | null;
  /** opened items, newest first; null until read */
  viewed: number[] | null;
  /** reads the session again (after signing in) */
  reload: () => void;
  signOut: () => Promise<void>;
  toggleFav: (id: number) => void;
  /** remembers an opened item: in the account when signed in, otherwise in this browser */
  recordView: (id: number) => void;
  clearViewed: () => Promise<void>;
  notify: (key: MessageKey, type?: "error") => void;
};

const Ctx = createContext<Session | null>(null);

export function AccountProvider({ children }: { children: React.ReactNode }) {
  const { settings, update } = useUi();
  const router = useRouter();
  // read inside the loader, which must not run again when a setting changes
  const ui = useRef({ settings, update });
  ui.current = { settings, update };
  const [me, setMe] = useState<Me | null>();
  const [favs, setFavs] = useState<Set<number> | null>(null);
  const [viewed, setViewed] = useState<number[] | null>(null);
  const [notice, setNotice] = useState<Notice>();
  const notify = useCallback(
    (key: MessageKey, type?: "error") =>
      setNotice((n) => ({ key, type, n: (n?.n ?? 0) + 1 })),
    [],
  );
  // the latest load; answers to older ones are dropped
  const loads = useRef(0);

  const load = useCallback(async () => {
    const gen = ++loads.current;
    // the first read shares the request other callers make on load; later ones (sign-in, sign-out) start fresh
    const user = await fetchAccount<Me>({ fresh: gen > 1 }).catch(() => null);
    if (gen !== loads.current) return;
    setMe(user);
    setPreferenceSync(false);
    if (!user) {
      setFavs(null);
      setViewed(loadViewed().map(([id]) => id));
      return;
    }
    // the account's settings replace this browser's; changes from now on are sent back
    const patch = accountPatch(user.preferences, ui.current.settings);
    if (Object.keys(patch).length) ui.current.update(patch);
    setPreferenceSync(true);
    // what this browser opened while signed out moves into the account
    const local = loadViewed();
    if (local.length)
      await api("/api/account/views", {
        method: "POST",
        json: { views: local.map(([id, at]) => ({ id, at })) },
      })
        .then(() => saveViewed([]))
        .catch(() => {});
    const [views, favourites] = await Promise.all(
      ["/api/account/views", "/api/account/favourites"].map((path) =>
        api<{ ids: number[] }>(path).catch(() => ({ ids: [] as number[] })),
      ),
    );
    if (gen !== loads.current) return;
    setViewed(views.ids);
    setFavs(new Set(favourites.ids));
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const value = useMemo<Session>(
    () => ({
      me,
      favs,
      viewed,
      notify,
      reload: () => {
        load();
        router.refresh();
      },
      signOut: async () => {
        await api("/api/account/logout", { method: "POST" }).catch(() => {});
        await load();
        // the favourites view belongs to the account
        if (new URLSearchParams(location.search).get("v") === "favourites")
          router.replace("/");
        else router.refresh();
      },
      toggleFav: (id) => {
        if (!favs) return notify("fav.needSignIn");
        const on = favs.has(id);
        const flip = (s: Set<number> | null, add: boolean) => {
          const next = new Set(s);
          if (add) next.add(id);
          else next.delete(id);
          return next;
        };
        setFavs((s) => flip(s, !on));
        api(`/api/account/favourites/${id}`, {
          method: on ? "DELETE" : "PUT",
        }).catch((e) => {
          setFavs((s) => flip(s, on));
          notify(
            e instanceof ApiError && e.status === 401
              ? "fav.needSignIn"
              : "fav.failed",
            "error",
          );
        });
      },
      recordView: (id) => {
        setViewed((v) => addViewed(v ?? [], id));
        if (me)
          api("/api/account/views", {
            method: "POST",
            json: { views: [{ id }] },
          }).catch(() => {});
        else recordLocalView(id);
      },
      clearViewed: async () => {
        try {
          if (me) await api("/api/account/views", { method: "DELETE" });
          else saveViewed([]);
          setViewed([]);
        } catch {
          notify("recent.clearFailed", "error");
        }
      },
    }),
    [me, favs, viewed, notify, load, router],
  );

  return (
    <Ctx value={value}>
      {children}
      {notice && <Toaster notice={notice} />}
    </Ctx>
  );
}

export function useAccount(): Session {
  const s = useContext(Ctx);
  if (!s) throw new Error("useAccount outside AccountProvider");
  return s;
}
