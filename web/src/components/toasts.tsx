"use client";
// The site's toasts. Astryx's useToast falls back to a viewport in a React root of its own when no ToastViewport
// is mounted above the caller; that root sits outside Providers, so its close button and region name were English
// in every language. The viewport is mounted here instead, inside Providers, and its code loads with the first
// toast rather than with the page. Components raise toasts with this module's useToast, not Astryx's.
// Toasts and the Back to top button share the bottom end of the window; while the button shows, the toasts sit above
// it (`toastClearance`), as a snackbar keeps clear of a floating action button.

import type { ToastOptions } from "@astryxdesign/core/Toast";
import dynamic from "next/dynamic";
import {
  createContext,
  use,
  useCallback,
  useMemo,
  useRef,
  useState,
} from "react";

const Viewport = dynamic(() => import("./toast-viewport"), { ssr: false });

type Toast = (options: ToastOptions) => void;

const Ctx = createContext<Toast | null>(null);

let clearance = 0;
const listeners = new Set<() => void>();
/** How far above the window's bottom edge the toasts sit, in px: set by the Back to top button while it shows
 *  (frame/back-to-top.tsx), 0 for the viewport's own place. */
export const toastClearance = {
  set(px: number) {
    if (px === clearance) return;
    clearance = px;
    for (const changed of listeners) changed();
  },
  get: () => clearance,
  subscribe(changed: () => void) {
    listeners.add(changed);
    return () => {
      listeners.delete(changed);
    };
  },
};

export function ToastHost({ children }: { children: React.ReactNode }) {
  // toasts raised before the viewport's code arrived; null until the first one
  const [queue, setQueue] = useState<ToastOptions[] | null>(null);
  // the viewport's own toast function, once it is mounted
  const ready = useRef<Toast | null>(null);
  const toast = useMemo<Toast>(
    () => (options) => {
      if (ready.current) ready.current(options);
      else setQueue((q) => [...(q ?? []), options]);
    },
    [],
  );
  const onReady = useCallback((show: Toast) => {
    ready.current = show;
  }, []);
  return (
    <Ctx value={toast}>
      {children}
      {queue && <Viewport queue={queue} onReady={onReady} />}
    </Ctx>
  );
}

/** Shows a toast (Astryx ToastOptions); one per `uniqueID`: raising it again replaces it. */
export function useToast(): Toast {
  const toast = use(Ctx);
  if (!toast) throw new Error("useToast outside Providers");
  return toast;
}
