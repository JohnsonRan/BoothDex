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

let loading: Promise<Turnstile> | undefined;

/** The Turnstile API, its script added on the first call; a failed load is tried again on the next. */
export function loadTurnstile(): Promise<Turnstile> {
  loading ??= new Promise<Turnstile>((resolve, reject) => {
    const script = document.createElement("script");
    script.src = SCRIPT;
    script.async = true;
    script.onload = () =>
      window.turnstile
        ? resolve(window.turnstile)
        : reject(new Error("Turnstile did not start"));
    script.onerror = () => {
      script.remove();
      reject(new Error("Turnstile did not load"));
    };
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

  // biome-ignore lint/correctness/useExhaustiveDependencies: attempt loads and renders the widget again after a failed load
  useEffect(() => {
    let gone = false;
    loadTurnstile().then(
      (api) => {
        if (gone || !ref.current) return;
        setFailed(false);
        widget.current = api.render(ref.current, {
          sitekey: siteKey,
          action,
          theme: dark ? "dark" : "light",
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
  }, [siteKey, action, dark, ui, attempt]);

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
