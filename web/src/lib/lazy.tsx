"use client";
// Lazy surfaces (dialogs, filter editors) keep their code out of the first load. Once the page has
// loaded and the browser is idle, their code is fetched anyway, so a first open does not wait for the network;
// pointing at or focusing a trigger starts it at once. A loaded surface renders directly: next/dynamic suspends
// once even for code already loaded, and React holds a revealed Suspense boundary back for up to 300 ms.
import { type ComponentType, useEffect, useState } from "react";

type Load = () => Promise<unknown>;

type Surface<P> = {
  /** starts loading the code (once); a failed fetch is tried again on the next call */
  preload: () => Promise<ComponentType<P>>;
  loaded?: ComponentType<P>;
};

/** A component whose code loads on `preload()` or on its first render through `Lazy`. */
export function lazySurface<P extends object>(
  load: () => Promise<ComponentType<P>>,
): Surface<P> {
  let loading: Promise<ComponentType<P>> | undefined;
  const surface: Surface<P> = {
    preload: () =>
      (loading ??= load().then(
        (c) => (surface.loaded = c),
        (e) => {
          loading = undefined;
          throw e;
        },
      )),
  };
  return surface;
}

/** Renders `surface` with the other props once its code has loaded, nothing until then. */
export function Lazy<P extends object>({
  surface,
  ...props
}: { surface: Surface<P> } & P) {
  const [Loaded, setLoaded] = useState(() => surface.loaded);
  useEffect(() => {
    if (!Loaded)
      surface.preload().then(
        (c) => setLoaded(() => c),
        () => {},
      );
  }, [Loaded, surface]);
  return Loaded ? <Loaded {...(props as unknown as P)} /> : null;
}

const start = (loads: Load[]) => {
  for (const load of loads) load().catch(() => {});
};

/** Loads `loads` after the page's load event, when the browser is idle. */
export function preloadWhenIdle(...loads: Load[]): void {
  const idle = () =>
    "requestIdleCallback" in window
      ? requestIdleCallback(() => start(loads), { timeout: 3000 })
      : setTimeout(() => start(loads), 500);
  if (document.readyState === "complete") idle();
  else addEventListener("load", idle, { once: true });
}

/** A trigger's props that start loading its surface on hover or focus. */
export const preloadOn = (...loads: Load[]) => ({
  onPointerEnter: () => start(loads),
  onFocus: () => start(loads),
});
