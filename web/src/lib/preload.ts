// Lazy surfaces (dialogs, editors) keep their code out of the first load. Once the page has loaded and the
// browser is idle, their code is fetched anyway, so a first open does not wait for the network; pointing at or
// focusing a trigger starts it at once. The bundler loads each chunk once, so repeated calls cost nothing.

type Load = () => Promise<unknown>;

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
