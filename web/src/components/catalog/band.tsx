"use client";
// The filter band: the toolbar row pinned under the top bar while the window scrolls, as a sticky child of a
// LayoutContent that does not scroll itself (LayoutContent.d.ts:28-33, its example :70-75; the style object as
// product-detail's sticky column, page.tsx:26-30, opaque as dashboard-alert-rail's sticky header, :895-900). It
// carries the page header's divider and, on that edge, the refresh progress (refresh.tsx).
// The avatar strip is the band's lower part: the band sticks a strip's shrink higher (`--strip-d`) with the toolbar
// pinned inside it, so once the toolbar reaches the top bar the strip goes on under it for that far while its
// cards shrink to 40 px, then stays, laid over the results that scroll beneath. The shrink is scroll-driven CSS
// (app/globals.css `strip-*`, transform and opacity only); nothing in the flow changes, so nothing below moves.
// The toggle at the row's end opens the shrunk strip over the results (the band sticks at the top bar's edge
// and the strip glides down to full size, lib/motion `glide`) and folds it back; while the strip is still at
// full size with the page it folds it as the scroll does, by scrolling there. A start past page 1 opens scrolled
// there (deep-start.ts).
// The state is the page's own, as the Collapsible blocks' custom disclosure (CollapsibleHookUsage.tsx:14-42: the
// open state, a trigger with aria-expanded and aria-controls, the content it shows).

import { Section } from "@astryxdesign/core/Section";
import { VStack } from "@astryxdesign/core/Stack";
import {
  type CSSProperties,
  type ReactNode,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { Avatar } from "@/lib/catalog";
import {
  cancelOnReducedMotion,
  glide,
  type Look,
  lookOf,
  prefersReducedMotion,
} from "@/lib/motion";
import { AvatarStrip } from "./avatar-strip";
import { RefreshProgress } from "./refresh";

// the top bar's height, which the AppShell keeps on its root for its own sticky parts
const TOP = "var(--_app-shell-header-height, 0px)";
// The band reaches up `--strip-d` (app/globals.css) past the top bar, under it: only its toolbar and its strip
// paint and take the pointer, each where it shows.
const HEAD: CSSProperties = {
  position: "sticky",
  insetBlockStart: `calc(${TOP} - var(--strip-d))`,
  zIndex: 1,
  pointerEvents: "none",
};
const OPEN: CSSProperties = { ...HEAD, insetBlockStart: TOP };
const BAR: CSSProperties = {
  position: "sticky",
  insetBlockStart: TOP,
  zIndex: 1,
  pointerEvents: "auto",
  backgroundColor: "var(--color-background-surface)",
  borderBlockEnd: "1px solid var(--color-border)",
};
// its own layer of the band, so the surface behind it stays behind its cards
const STRIP: CSSProperties = { position: "relative", isolation: "isolate" };
// How far the strip goes under the toolbar: its top padding, its label line and the label's gap, then the
// cards' shrink from 104 to 40 px less the 8 px a shrunk card keeps from the toolbar. Registered as a length
// (app/globals.css), so it reads back in px.
const STRIP_D =
  "calc(var(--spacing-4) + var(--text-body-leading) * var(--text-body-size) + var(--spacing-3) + 4rem - var(--spacing-2))";

let band: HTMLElement | null = null;
let bar: HTMLElement | null = null;

/**
 * The viewport px above the line the band's bottom edge holds once stuck with its strip shrunk (opened by hand or
 * not, so a place saved and the same place restored agree): content above it is under the top bar or the band.
 * Scrolling does not change it, so a place measured from it is found again at the same scroll. deep-start.ts
 * reads the same line before the page hydrates.
 */
export function covered() {
  if (!band || !bar) return 0;
  const head = getComputedStyle(band);
  return (
    Number.parseFloat(getComputedStyle(bar).top) -
    (Number.parseFloat(head.getPropertyValue("--strip-d")) || 0) +
    band.getBoundingClientRect().height
  );
}

export function FilterBand({
  label,
  toolbar,
  avatars,
  folded = false,
  children,
}: {
  label: string;
  /** the filter row and the end cluster; none in the recent view */
  toolbar?: ReactNode;
  /** the avatar strip's avatars, in the items view */
  avatars?: Avatar[];
  /** the page opens scrolled past the strip (a start past page 1): the strip is shrunk from the first paint */
  folded?: boolean;
  /** the results */
  children: ReactNode;
}) {
  const ref = useRef<HTMLElement>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const stripRef = useRef<HTMLDivElement>(null);
  const id = useId();
  // the strip shrunk all the way with the scroll
  const [shrunk, setShrunk] = useState(folded);
  // opened over the results by hand
  const [open, setOpen] = useState(false);
  // the scroll the shrink starts at and its length; the look a hand turn glides from
  const span = useRef({ start: 0, length: 0, shrunk: folded });
  const was = useRef<Look | null>(null);
  const openNow = useRef(open);
  openNow.current = open;

  useLayoutEffect(() => {
    band = ref.current;
    bar = barRef.current;
    return () => {
      band = null;
      bar = null;
    };
  }, []);

  useEffect(() => {
    const head = ref.current;
    const b = barRef.current;
    if (!avatars || !head || !b) return;
    // Firefox has no scroll-driven animations: the same keyframes run paused at the progress set here
    const driven = CSS.supports("animation-timeline", "scroll()");
    let frame = 0;
    const run = () => {
      frame = 0;
      const { start, length } = span.current;
      const p = length
        ? Math.min(1, Math.max(0, (scrollY - start) / length))
        : 0;
      if (!driven) head.style.setProperty("--strip-p", String(p));
      // the shrink runs from the first measure on; reduced motion shows the strip whole or shrunk (app/globals.css)
      const state = p <= 0 ? "none" : p >= 1 ? "all" : "part";
      if (head.dataset.shrink !== state) head.dataset.shrink = state;
      span.current.shrunk = p >= 1;
      setShrunk(p >= 1);
      // back at full size with the page, an opened strip is the same as one that is not
      if (p <= 0) setOpen(false);
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(run);
    };
    // where the toolbar meets the top bar: the band's place in the flow, less the top bar
    const measure = () => {
      const parent = head.parentElement;
      if (!parent) return;
      const start =
        parent.getBoundingClientRect().top +
        scrollY -
        Number.parseFloat(getComputedStyle(b).top);
      const length =
        Number.parseFloat(
          getComputedStyle(head).getPropertyValue("--strip-d"),
        ) || 0;
      if (start !== span.current.start)
        head.style.setProperty("--strip-y0", `${start}px`);
      span.current = { ...span.current, start, length };
      schedule();
    };
    measure();
    addEventListener("scroll", schedule, { passive: true });
    // what is above the band changes its place; WebKit sends no scroll for a deep start's jump
    const sized = new ResizeObserver(measure);
    sized.observe(document.body);
    return () => {
      cancelAnimationFrame(frame);
      removeEventListener("scroll", schedule);
      sized.disconnect();
    };
  }, [avatars]);

  // a hand turn glides from the look before it; the first render and a turn by the scroll stay as they are
  useLayoutEffect(() => {
    const from = was.current;
    const s = stripRef.current;
    was.current = null;
    if (!from || !s) return;
    return cancelOnReducedMotion(glide(s, from, open));
  }, [open]);

  const turn = useCallback((next: boolean) => {
    const s = stripRef.current;
    if (s) was.current = lookOf(s, s.querySelectorAll<HTMLElement>(PARTS));
    setOpen(next);
  }, []);
  const toggle = useCallback(() => {
    if (open) turn(false);
    else if (span.current.shrunk) turn(true);
    else {
      // still at full size with the page: fold it as the scroll does, by going there (a whole px on, as an
      // engine may stop a smooth scroll short of a fraction)
      const { start, length } = span.current;
      scrollTo({
        top: Math.ceil(start + length),
        behavior: prefersReducedMotion() ? "instant" : "smooth",
      });
    }
  }, [open, turn]);
  const picked = useCallback(() => {
    if (openNow.current) turn(false);
  }, [turn]);

  const strip = useMemo(
    () =>
      avatars && (
        <AvatarStrip
          avatars={avatars}
          expanded={open || !shrunk}
          controls={id}
          onToggle={toggle}
          onPick={picked}
        />
      ),
    [avatars, open, shrunk, id, toggle, picked],
  );

  return (
    <VStack gap={0}>
      <Section
        ref={ref}
        aria-label={label}
        variant="transparent"
        padding={0}
        data-strip-open={open ? "" : undefined}
        // a start past page 1 is shrunk from the first paint; page 1 shrinks once measured
        data-shrink={avatars && folded ? "all" : undefined}
        style={
          avatars
            ? ({
                ...(open ? OPEN : HEAD),
                "--strip-d": STRIP_D,
                "--strip-p": folded ? 1 : 0,
              } as CSSProperties)
            : ({ ...HEAD, "--strip-d": "0px" } as CSSProperties)
        }
      >
        <VStack
          ref={barRef}
          padding={toolbar ? 6 : 0}
          paddingBlockStart={toolbar ? 4 : 0}
          className="filter-toolbar"
          style={BAR}
        >
          <VStack gap={4}>{toolbar}</VStack>
          <RefreshProgress />
        </VStack>
        {strip && (
          // a VStack's padding, not a Section's: a Carousel bleeds into a Section's inset
          <VStack
            ref={stripRef}
            id={id}
            data-strip=""
            paddingInline={6}
            paddingBlockStart={4}
            paddingBlockEnd={2}
            style={STRIP}
          >
            <div className="strip-surface" aria-hidden />
            {strip}
          </VStack>
        )}
      </Section>
      <VStack
        data-band-content=""
        gap={4}
        padding={6}
        paddingBlockStart={avatars ? 2 : 4}
        style={{
          // the results keep their own place through every change (result-pages.tsx), in every engine alike
          overflowAnchor: "none",
          // room to scroll the strip shrunk however few the results
          minBlockSize: avatars ? `calc(100lvh - ${TOP})` : undefined,
        }}
      >
        {children}
      </VStack>
    </VStack>
  );
}

/** what the scroll shrinks or fades in the strip (app/globals.css), each glided by a hand turn */
const PARTS = ".strip-thumb, .strip-label, .strip-surface";
