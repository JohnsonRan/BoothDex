// Cloudflare Turnstile, rendered explicitly (developers.cloudflare.com/turnstile/get-started/client-side-rendering).
// The script loads once, on the first form that needs it, never with the page. A token is single-use: the form
// resets its widget after every attempt, accepted or not.

import { useMediaQuery } from "@astryxdesign/core/hooks";
import { useEffect, useEffectEvent, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import type { UiLang } from "./messages";

const SCRIPT =
  "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";

type RenderOptions = {
  sitekey: string;
  action: string;
  theme: "light" | "dark";
  size: "flexible" | "compact";
  language: string;
  callback: (token: string) => void;
  "expired-callback": () => void;
  "error-callback": () => void;
};

type Turnstile = {
  render: (container: HTMLElement, options: RenderOptions) => string;
  reset: (widgetId: string) => void;
  remove: (widgetId: string) => void;
};

declare global {
  interface Window {
    turnstile?: Turnstile;
  }
}

/** Turnstile's own language codes for the interface languages. */
export const TURNSTILE_LANGUAGES: Record<UiLang, string> = {
  zh: "zh-cn",
  en: "en",
  ja: "ja",
  ko: "ko",
};

/** The narrowest host the flexible widget fits: Cloudflare gives its frame `min-width: 300px`, so a narrower host (the
 *  guestbook drawer on a phone) would clip its right edge. The compact widget is 150 px wide. */
const FLEXIBLE_MIN = 300;

/** How long the script may take before it counts as not loaded: a network that drops the request fires neither `load`
 *  nor `error`, and the form would wait for a widget that never comes instead of letting the server decide. */
export const LOAD_TIMEOUT_MS = 10_000;

let loading: Promise<Turnstile> | undefined;

/** The Turnstile API, its script added on the first call; a failed load is tried again on the next. */
export function loadTurnstile(): Promise<Turnstile> {
  loading ??= new Promise<Turnstile>((resolve, reject) => {
    // a script that arrived after its load timed out has already started Turnstile
    if (window.turnstile) return resolve(window.turnstile);
    const script = document.createElement("script");
    const fail = (why: string) => {
      clearTimeout(timer);
      script.onload = script.onerror = null;
      script.remove();
      reject(new Error(why));
    };
    const timer = setTimeout(
      () => fail("Turnstile timed out"),
      LOAD_TIMEOUT_MS,
    );
    script.src = SCRIPT;
    script.async = true;
    script.referrerPolicy = "strict-origin-when-cross-origin";
    script.onload = () => {
      clearTimeout(timer);
      if (window.turnstile) resolve(window.turnstile);
      else reject(new Error("Turnstile did not start"));
    };
    script.onerror = () => fail("Turnstile did not load");
    document.head.append(script);
  }).catch((e) => {
    loading = undefined;
    throw e;
  });
  return loading;
}

/** A widget for `siteKey` in the element `ref` is attached to, in the site's colour mode and language. `token` is
 *  empty until the visitor passes, and again once it expires or `reset` is called. */
export function useTurnstile(siteKey: string, action: string) {
  const { ui, settings } = useUi();
  const prefersDark = useMediaQuery("(prefers-color-scheme: dark)");
  const dark =
    settings.theme === "dark" || (settings.theme === "system" && prefersDark);
  const ref = useRef<HTMLElement>(null);
  const widget = useRef<string | undefined>(undefined);
  const [token, setToken] = useState("");
  const [failed, setFailed] = useState(false);
  // bumped by `retry`: the drawer keeps the form mounted while it is closed, so reopening it loads nothing
  const [attempt, setAttempt] = useState(0);
  const take = useEffectEvent((t: string) => setToken(t));
  // null until the host has been laid out; a hidden host (width 0, the closed drawer keeps its form) changes nothing
  const [compact, setCompact] = useState<boolean | null>(null);
  const hostWidth = useRef(0);
  // A new size renders the widget again, which drops its token: while the visitor holds one (a scrollbar appearing
  // as messages load can narrow the host), the size waits until the token is used, expires or is reset.
  const fit = useEffectEvent(() => {
    if (hostWidth.current > 0 && !token)
      setCompact(hostWidth.current < FLEXIBLE_MIN);
  });

  // biome-ignore lint/correctness/useExhaustiveDependencies: a failed load replaces the host with a Banner, and Try again brings a new one
  useEffect(() => {
    const host = ref.current;
    if (!host) return;
    const sized = new ResizeObserver(([entry]) => {
      hostWidth.current = entry.contentRect.width;
      fit();
    });
    sized.observe(host);
    return () => sized.disconnect();
  }, [failed]);

  useEffect(() => {
    if (!token) fit();
  }, [token]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: attempt loads and renders the widget again after a failed load
  useEffect(() => {
    if (compact === null) return;
    let gone = false;
    loadTurnstile().then(
      (api) => {
        if (gone || !ref.current) return;
        setFailed(false);
        widget.current = api.render(ref.current, {
          sitekey: siteKey,
          action,
          theme: dark ? "dark" : "light",
          // as wide as the form's fields where they leave it room
          size: compact ? "compact" : "flexible",
          language: TURNSTILE_LANGUAGES[ui],
          callback: take,
          "expired-callback": () => take(""),
          "error-callback": () => take(""),
        });
      },
      () => !gone && setFailed(true),
    );
    return () => {
      gone = true;
      if (widget.current) window.turnstile?.remove(widget.current);
      widget.current = undefined;
      setToken("");
    };
  }, [siteKey, action, dark, ui, attempt, compact]);

  const reset = () => {
    if (widget.current) window.turnstile?.reset(widget.current);
    setToken("");
  };
  const retry = () => {
    setFailed(false);
    setAttempt((n) => n + 1);
  };
  return { ref, token, failed, reset, retry };
}

export type TurnstileState = ReturnType<typeof useTurnstile>;
