"use client";
// The admin session: probes /api/admin/overview, then keeps the overview live over the SSE stream (one event
// every 2 s; EventSource reconnects by itself). Until the probe says the visitor is an admin, every page's frame
// shows a state in place of its content (page-frame.tsx): checking, sign in (the public site's dialog), not an
// admin, or unreachable.

import { Button } from "@astryxdesign/core/Button";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { Spinner } from "@astryxdesign/core/Spinner";
import dynamic from "next/dynamic";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import { useUi } from "@/components/providers";
import { ApiError, api } from "@/lib/api";
import { AdminShell } from "./admin-shell";
import { type AdminKey, useAdminT } from "./messages";
import type { Overview } from "./types";

const SignInDialog = dynamic(
  () => import("@/components/frame/sign-in").then((m) => m.SignInDialog),
  { ssr: false },
);

type Live = { overview: Overview; connected: boolean };
const LiveContext = createContext<Live | null>(null);

const GateContext = createContext<React.ReactNode>(null);

/** What a page frame shows in place of its content until the visitor is a signed-in admin; null after. */
export const useGate = () => useContext(GateContext);

/** The latest overview and whether the stream is connected; only inside a signed-in admin page. */
export function useLive(): Live {
  const live = useContext(LiveContext);
  if (!live) throw new Error("useLive outside AdminConsole");
  return live;
}

type Gate = "checking" | "login" | "forbidden" | "failed";
const GATE: Record<
  Exclude<Gate, "checking">,
  { title: AdminKey; desc?: AdminKey; icon: `admin:${string}` }
> = {
  login: {
    title: "gate.signInTitle",
    desc: "gate.signInDesc",
    icon: "admin:signIn",
  },
  forbidden: {
    title: "gate.forbiddenTitle",
    desc: "gate.forbiddenDesc",
    icon: "admin:forbidden",
  },
  failed: { title: "gate.failedTitle", icon: "admin:offline" },
};

export function AdminConsole({
  mobile,
  children,
}: {
  /** the request's device hint, so the server renders the phone shell on a phone */
  mobile: boolean;
  children: React.ReactNode;
}) {
  const t = useAdminT();
  const ui = useUi();
  const [gate, setGate] = useState<Gate>("checking");
  const [live, setLive] = useState<Live | null>(null);
  const [signIn, setSignIn] = useState<boolean>();

  const probe = useCallback(() => {
    api<Overview>("/api/admin/overview").then(
      (overview) => setLive({ overview, connected: false }),
      (e) => {
        const status = e instanceof ApiError ? e.status : 0;
        setGate(
          status === 401 ? "login" : status === 403 ? "forbidden" : "failed",
        );
      },
    );
  }, []);
  useEffect(() => probe(), [probe]);

  const signedIn = live !== null;
  useEffect(() => {
    if (!signedIn) return;
    const es = new EventSource("/api/admin/stream");
    es.addEventListener("overview", (e) =>
      setLive({ overview: JSON.parse(e.data), connected: true }),
    );
    es.onerror = () => setLive((l) => l && { ...l, connected: false });
    return () => es.close();
  }, [signedIn]);

  if (live)
    return (
      <AdminShell mobile={mobile}>
        <LiveContext value={live}>{children}</LiveContext>
      </AdminShell>
    );
  const state = gate === "checking" ? null : GATE[gate];
  return (
    <AdminShell mobile={mobile}>
      <GateContext
        value={
          state ? (
            <EmptyState
              headingLevel={2}
              title={t(state.title)}
              description={state.desc && t(state.desc)}
              icon={<Icon icon={state.icon} size="lg" />}
              actions={
                gate === "login" || gate === "forbidden" ? (
                  <Button
                    label={ui.t("nav.signIn")}
                    variant="primary"
                    onClick={() => setSignIn(true)}
                  />
                ) : (
                  <Button label={t("gate.retry")} onClick={probe} />
                )
              }
            />
          ) : (
            <Spinner label={t("gate.checking")} />
          )
        }
      >
        {children}
      </GateContext>
      {signIn !== undefined && (
        <SignInDialog
          isOpen={signIn}
          onOpenChange={setSignIn}
          gate={{
            title: t("gate.signInTitle"),
            description: t("gate.signInDesc"),
          }}
          onSignedIn={() => {
            setSignIn(false);
            setGate("checking");
            probe();
          }}
        />
      )}
    </AdminShell>
  );
}
