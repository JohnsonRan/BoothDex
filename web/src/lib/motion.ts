// Motion values and the entrance itself (the pattern of doona's apps/lite/src/ui/motion). The easing is Astryx's
// `--ease-standard`; every animation on the site reads these, so retuning one number retunes it everywhere.
// Only transform and opacity move, and reduced motion leaves everything static.

const EASE = "cubic-bezier(0.24, 1, 0.4, 1)";

const MOTION = {
  /** a page section rises this far and fades in */
  pageRise: 8,
  pageDuration: 360,
  pageStagger: 50,
  /** sections after the sixth share the sixth step's delay */
  pageSteps: 6,
  /** grid tiles and avatar cards: a smaller section entrance */
  rowRise: 6,
  rowDuration: 280,
  rowStagger: 30,
  /** rows start this long after their section */
  rowLead: 120,
  /** rows past this many appear at once (the server renders the first 24 tiles) */
  rowMax: 24,
  /** content a slow refresh replaced fades in where the old content was */
  swapDuration: 240,
  /** the avatar strip opened or folded by hand */
  turnDuration: 240,
} as const;

/** `Animation.id`s (tools/motion.cjs matches them), so checks can tell the entrance from a component's own motion. */
const PAGE_ENTER_ID = "page-enter";
const ROW_ENTER_ID = "row-enter";
const SWAP_ID = "swap-in";
const TURN_ID = "strip-turn";

/** Marks a container whose children (a carousel: its slides) enter as rows: the grid, the avatar strip. */
export const ROWS = { "data-enter-rows": "" } as const;
/** Marks a loading silhouette: the content that replaces it enters, the silhouette itself stays static. */
export const PLACEHOLDER = { "data-placeholder": "" } as const;

const QUERY = "(prefers-reduced-motion: reduce)";

export const prefersReducedMotion = () => matchMedia(QUERY).matches;

/** Cancels `running` (each jumps to its end) now and whenever reduced motion turns on; returns the cleanup. */
export function cancelOnReducedMotion(running: Animation[]): () => void {
  const list = matchMedia(QUERY);
  const cancel = () => {
    for (const a of running) a.cancel();
  };
  const onChange = () => list.matches && cancel();
  list.addEventListener("change", onChange);
  return () => {
    list.removeEventListener("change", onChange);
    cancel();
  };
}

const rise = (el: Element, id: string, px: number, ms: number, delay: number) =>
  el.animate(
    [
      { opacity: 0, transform: `translateY(${px}px)` },
      { opacity: 1, transform: "none" },
    ],
    { id, duration: ms, delay, easing: EASE, fill: "backwards" },
  );

const onScreen = (el: Element) => {
  const box = el.getBoundingClientRect();
  return box.height > 0 && box.bottom > 0 && box.top < innerHeight;
};

/**
 * Content that replaced a painted placeholder enters: each arrived root rises as a page section, a stagger
 * apart and in reading order; rows inside it (children of a `ROWS` container on screen) follow `rowLead` after
 * their section. Roots off screen appear at once.
 */
export function enter(roots: Element[]): Animation[] {
  const running: Animation[] = [];
  const sections = roots
    .filter(onScreen)
    .sort(
      (a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top,
    );
  sections.forEach((root, i) => {
    const delay = Math.min(i, MOTION.pageSteps - 1) * MOTION.pageStagger;
    running.push(
      rise(root, PAGE_ENTER_ID, MOTION.pageRise, MOTION.pageDuration, delay),
    );
    const groups = [
      ...(root.matches("[data-enter-rows]") ? [root] : []),
      ...root.querySelectorAll("[data-enter-rows]"),
    ];
    for (const group of groups) {
      const slides = group.querySelectorAll('[aria-roledescription="slide"]');
      const rows = [...(slides.length ? slides : group.children)]
        .slice(0, MOTION.rowMax)
        .filter(onScreen);
      for (const [n, row] of rows.entries())
        running.push(
          rise(
            row,
            ROW_ENTER_ID,
            MOTION.rowRise,
            MOTION.rowDuration,
            delay + MOTION.rowLead + n * MOTION.rowStagger,
          ),
        );
    }
  });
  return running;
}

/** The fade of content a slow refresh replaced (catalog/refresh.tsx Swap): opacity only, nothing moves. */
export const swapIn = (el: Element): Animation[] =>
  prefersReducedMotion()
    ? []
    : [
        el.animate([{ opacity: 0 }, { opacity: 1 }], {
          id: SWAP_ID,
          duration: MOTION.swapDuration,
          easing: EASE,
        }),
      ];

/**
 * Each part's scroll morph (its CSS animation), its keyframes resolved as the part stands now. One query for the
 * page: `getAnimations` on an element runs a lifecycle update each call, which over a strip of cards cost most of a
 * click.
 */
export const morphsOf = (parts: HTMLElement[]) => {
  const morphs = new Map<HTMLElement, ComputedKeyframe[]>();
  const wanted = new Set(parts);
  for (const a of document.getAnimations()) {
    if (!(a instanceof CSSAnimation && a.effect instanceof KeyframeEffect))
      continue;
    const el = a.effect.target;
    if (el instanceof HTMLElement && wanted.has(el) && !morphs.has(el))
      morphs.set(el, a.effect.getKeyframes());
  }
  return morphs;
};

/**
 * The avatar strip turned by hand (catalog/band/band.tsx) along the scroll's path: each part runs its scroll morph's
 * keyframes over time (`morphs`, to their end folding, back to their start growing or opening), while `slides` (the
 * strip, and in place the results after it) move down from `from` to `to` px, in step. Each holds its end until
 * cancelled, so the layout can take over in the same frame.
 */
export function turnAlong(
  slides: HTMLElement[],
  [from, to]: [number, number],
  morphs: ReturnType<typeof morphsOf>,
  folding: boolean,
): Animation[] {
  if (prefersReducedMotion()) return [];
  const timing = {
    id: TURN_ID,
    duration: MOTION.turnDuration,
    easing: EASE,
    fill: "forwards",
  } as const;
  const running = slides.map((el) =>
    el.animate(
      [{ translate: `0 ${from}px` }, { translate: `0 ${to}px` }],
      timing,
    ),
  );
  for (const [el, frames] of morphs) {
    const path = frames.map(({ computedOffset, ...k }) => ({
      ...k,
      offset: folding ? computedOffset : 1 - computedOffset,
    }));
    running.push(el.animate(folding ? path : path.reverse(), timing));
  }
  return running;
}
