"use client";
// The filter band: the toolbar row pinned under the top bar while the window scrolls, as a sticky child of a
// LayoutContent that does not scroll itself (LayoutContent.d.ts:28-33, its example :70-75; the style object as
// product-detail's sticky column, page.tsx:26-30, opaque as dashboard-alert-rail's sticky header, :895-900). It
// carries the page header's divider and, on that edge, the refresh progress (refresh.tsx).
// The avatar strip is the band's lower part: the band sticks a strip's shrink higher (`--strip-d`) with the toolbar
// pinned inside it, so once the toolbar reaches the top bar the strip goes on under it for that far while its
// cards morph into a row of chips the height of the toolbar's controls (avatar-strip.tsx), then stays, laid over
// the results that scroll beneath. The morph is scroll-driven CSS (app/globals.css `strip-*`, transform and
// opacity only); nothing in the flow changes, so nothing below moves. The band tells the strip where the scroll
// has it, so shrunk and in use it lays its chips out for real.
// The toggle in the toolbar's end cluster (strip-toggle.tsx, handed the strip's state here) opens the shrunk strip
// over the results (the band sticks at the top bar's edge and the strip glides down to full size along the
// morph, lib/motion `glide`) and folds it back. At full size in its place it folds the strip there (`tucked`,
// lib/motion `turnInPlace`): only the chip row stays in the flow, the results slide up, the band has no shrink
// (`--strip-d` 0, as covered() and changeTop() read it) and the scroll morphs nothing. Expanded, it grows back in
// place, or, scrolled past, comes back with the window as far down and opens over the results. Part-way shrunk,
// it scrolls to the shrink's end. A start past page 1 opens scrolled there (deep-start.ts).
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
import { flushSync } from "react-dom";
import type { Avatar } from "@/lib/catalog";
import {
  cancelOnReducedMotion,
  glide,
  type Look,
  lookOf,
  morphsOf,
  prefersReducedMotion,
  turnInPlace,
} from "@/lib/motion";
import {
  AvatarStrip,
  CARD_REM,
  CHIP_REM,
  type StripMorph,
} from "./avatar-strip";
import { attachBand, attachBar } from "./band-edge";
import { RefreshProgress } from "./refresh";
import { StripToggleContext, type StripToggleState } from "./strip-toggle";

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
// A card's top to its chip's: a chip ends where its card's caption does (the card, the caption's gap and line).
const STRIP_DY = `calc(${CARD_REM - CHIP_REM}rem + var(--spacing-2) + var(--text-supporting-leading) * var(--text-supporting-size))`;
// How far the strip goes under the toolbar: its top padding, its label line and the label's gap, then a card's
// top to its chip's less the 8 px a chip keeps from the toolbar, as many as the strip keeps below it. Registered
// as a length (app/globals.css), so it reads back in px.
const STRIP_D = `calc(var(--spacing-4) + var(--text-body-leading) * var(--text-body-size) + var(--spacing-3) + ${STRIP_DY} - var(--spacing-2))`;
// the chip's scale of the card (app/globals.css `strip-thumb`)
const STRIP_SCALE = CHIP_REM / CARD_REM;
// Folded in place, the strip keeps its shrink to itself and reaches that far up under the toolbar, as the scroll
// takes it, so the flow keeps only its chip row.
const TUCKED = {
  ...STRIP,
  "--strip-d": STRIP_D,
  marginBlockStart: "calc(-1 * var(--strip-d))",
} as CSSProperties;

export function FilterBand({
  label,
  toolbar,
  avatars,
  folded: foldedNow = false,
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
  const resultsRef = useRef<HTMLDivElement>(null);
  const id = useId();
  // How the server drew the first paint. The band takes over from there and sets its marks itself (`data-morph`,
  // `data-shrink`, the strip's `data-chips`), so a later render with another page (a filter change from a start
  // past page 1, which goes to page 1) must not take them back.
  const [folded] = useState(foldedNow);
  // the strip shrunk all the way with the scroll
  const [shrunk, setShrunk] = useState(folded);
  // opened over the results by hand
  const [open, setOpen] = useState(false);
  // folded in place by hand: shrunk, out of the scroll's hands
  const [tucked, setTucked] = useState(false);
  // the scroll the shrink starts at and its length; the look a hand turn glides from
  const span = useRef({ start: 0, length: 0, shrunk: folded });
  const was = useRef<Look | null>(null);
  const openNow = useRef(open);
  openNow.current = open;
  const morph = useRef<StripMorph>(null);
  // a hand turn's glide while it runs: the strip is not at rest until it ends
  const gliding = useRef<Animation[] | null>(null);
  // where the strip stood before it folded or grew in place (and its morphs, folding), and that turn while it
  // runs: the scroll's marks wait for its end
  const placed = useRef<{
    top: number;
    morphs?: ReturnType<typeof morphsOf>;
  } | null>(null);
  const turning = useRef<Animation[] | null>(null);
  const rerun = useRef(() => {});

  // covered() reads these as they attach, not from a layout effect of the band's: the results are its children,
  // whose layout effects run first when the two mount together (a client navigation back to the results)
  const onBand = useCallback((el: HTMLElement | null) => {
    ref.current = el;
    attachBand(el);
  }, []);
  const onBar = useCallback((el: HTMLDivElement | null) => {
    barRef.current = el;
    attachBar(el);
  }, []);

  // each server render sends the avatars as a new array: the same ones keep the strip and its observers as they are
  const avatarsKey = JSON.stringify(avatars);
  const shown = useMemo<Avatar[] | undefined>(
    () => avatarsKey && JSON.parse(avatarsKey),
    [avatarsKey],
  );
  const hasStrip = !!shown;

  useEffect(() => {
    const head = ref.current;
    const b = barRef.current;
    if (!hasStrip || !head || !b) return;
    // Firefox has no scroll-driven animations: the same keyframes run paused at the progress set here
    const driven = CSS.supports("animation-timeline", "scroll()");
    let frame = 0;
    const run = () => {
      frame = 0;
      const { start, length } = span.current;
      // folded in place, the strip is shrunk wherever the scroll is
      const p = tucked
        ? 1
        : length
          ? Math.min(1, Math.max(0, (scrollY - start) / length))
          : 0;
      if (!driven) head.style.setProperty("--strip-p", String(p));
      // the shrink runs from the first measure on (`data-morph`); where the scroll has it (`data-shrink`) hides the
      // Carousel's buttons and, shrunk, draws the strip as the morph ends (app/globals.css); a turn in place is
      // part-way until it ends, as the scroll would be
      const state = turning.current
        ? "part"
        : p <= 0
          ? "none"
          : p >= 1
            ? "all"
            : "part";
      head.dataset.morph ??= "";
      if (head.dataset.shrink !== state) head.dataset.shrink = state;
      span.current.shrunk = p >= 1;
      setShrunk(p >= 1);
      morph.current?.settle(
        p <= 0
          ? "none"
          : p >= 1 && !openNow.current && !gliding.current && !turning.current
            ? "rest"
            : "part",
      );
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
    rerun.current = schedule;
    addEventListener("scroll", schedule, { passive: true });
    // a new size of what is above the band in the flow (the top bar, the page header) moves it; what is below it,
    // the results, does not
    const sized = new ResizeObserver(measure);
    for (
      let e = head.parentElement;
      e && e !== document.body;
      e = e.parentElement
    )
      for (let s = e.previousElementSibling; s; s = s.previousElementSibling)
        sized.observe(s);
    return () => {
      cancelAnimationFrame(frame);
      removeEventListener("scroll", schedule);
      sized.disconnect();
    };
  }, [hasStrip, tucked]);

  // a hand turn glides from the look before it; the first render and a turn by the scroll stay as they are. Folded
  // back, the strip is at rest once the glide has drawn it.
  useLayoutEffect(() => {
    const from = was.current;
    const s = stripRef.current;
    was.current = null;
    if (!from || !s) return;
    const running = glide(s, from, open);
    gliding.current = running;
    Promise.allSettled(running.map((a) => a.finished)).then(() => {
      if (gliding.current !== running) return;
      gliding.current = null;
      if (span.current.shrunk && !openNow.current)
        morph.current?.settle("rest");
    });
    return cancelOnReducedMotion(running);
  }, [open]);

  // folded or grown back in place by hand: the strip runs the scroll's morph from where it stood and the results
  // slide with it. Folded, the strip is at rest once the morph has drawn it, and in use: the toggle was pressed.
  useLayoutEffect(() => {
    const from = placed.current;
    const s = stripRef.current;
    const results = resultsRef.current;
    placed.current = null;
    turning.current = null;
    if (!from || !s || !results) return;
    const running = turnInPlace(
      s,
      from.top,
      from.morphs ?? morphsOf(partsOf(s)),
      results,
      tucked,
    );
    turning.current = running;
    Promise.allSettled(running.map((a) => a.finished)).then(() => {
      if (turning.current !== running) return;
      turning.current = null;
      rerun.current();
      if (tucked) {
        morph.current?.settle("rest");
        morph.current?.use();
      }
    });
    return cancelOnReducedMotion(running);
  }, [tucked]);

  const turn = useCallback((next: boolean) => {
    const s = stripRef.current;
    // opened from the chips: the cards drawn as the chips are, which the glide takes to full size
    if (next) morph.current?.settle("part");
    if (s) was.current = lookOf(s, partsOf(s));
    setOpen(next);
  }, []);
  const tuck = useCallback((next: boolean) => {
    const s = stripRef.current;
    // folded, the parts drop their morphs: taken now
    if (s)
      placed.current = {
        top: s.getBoundingClientRect().top,
        morphs: next ? morphsOf(partsOf(s)) : undefined,
      };
    setTucked(next);
  }, []);
  const toggle = useCallback(() => {
    if (open) turn(false);
    else if (tucked) {
      // the chips go back to the cards' row, drawn as they are
      morph.current?.settle("part");
      if (scrollY <= span.current.start) tuck(false);
      else {
        // scrolled past: the shrink goes back in the flow above the tiles and the window as far down, in one frame
        const results = resultsRef.current;
        const was = results?.getBoundingClientRect().top ?? 0;
        flushSync(() => {
          setTucked(false);
          turn(true);
        });
        scrollBy({
          top: (results?.getBoundingClientRect().top ?? 0) - was,
          behavior: "instant",
        });
      }
    } else if (span.current.shrunk) turn(true);
    else if (scrollY <= span.current.start) tuck(true);
    else {
      // part-way: fold it as the scroll does, by going on to the shrink's end (a whole px on, as an engine may
      // stop a smooth scroll short of a fraction)
      const { start, length } = span.current;
      scrollTo({
        top: Math.ceil(start + length),
        behavior: prefersReducedMotion() ? "instant" : "smooth",
      });
    }
  }, [open, tucked, turn, tuck]);
  const picked = useCallback(() => {
    if (openNow.current) turn(false);
  }, [turn]);

  const strip = useMemo(
    () =>
      shown && (
        <AvatarStrip
          avatars={shown}
          controls={id}
          onPick={picked}
          folded={folded}
          morph={morph}
        />
      ),
    [shown, id, picked, folded],
  );
  const expanded = open || (!tucked && !shrunk);
  const toggleState = useMemo<StripToggleState | null>(
    () => (hasStrip ? { expanded, controls: id, onToggle: toggle } : null),
    [hasStrip, expanded, id, toggle],
  );

  return (
    <VStack gap={0}>
      <Section
        ref={onBand}
        aria-label={label}
        variant="transparent"
        padding={0}
        data-strip-open={open ? "" : undefined}
        data-strip-tucked={tucked ? "" : undefined}
        // a start past page 1 is shrunk from the first paint; page 1 shrinks once measured
        data-morph={avatars && folded ? "" : undefined}
        data-shrink={avatars && folded ? "all" : undefined}
        style={
          avatars
            ? ({
                ...(open ? OPEN : HEAD),
                "--strip-d": tucked ? "0px" : STRIP_D,
                "--strip-dy": STRIP_DY,
                "--strip-scale": STRIP_SCALE,
                "--strip-p": folded ? 1 : 0,
              } as CSSProperties)
            : ({ ...HEAD, "--strip-d": "0px" } as CSSProperties)
        }
      >
        <VStack
          ref={onBar}
          padding={toolbar ? 6 : 0}
          paddingBlockStart={toolbar ? 4 : 0}
          className="filter-toolbar"
          style={BAR}
        >
          <VStack gap={4}>
            <StripToggleContext value={toggleState}>
              {toolbar}
            </StripToggleContext>
          </VStack>
          <RefreshProgress />
        </VStack>
        {strip && (
          // a VStack's padding, not a Section's: a Carousel bleeds into a Section's inset
          <VStack
            ref={stripRef}
            data-strip=""
            paddingInline={6}
            paddingBlockStart={4}
            paddingBlockEnd={2}
            style={tucked ? TUCKED : STRIP}
          >
            <VStack className="strip-surface" aria-hidden />
            {strip}
          </VStack>
        )}
      </Section>
      <VStack
        ref={resultsRef}
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

/** what the scroll morphs or fades in the strip (app/globals.css), each glided by a hand turn */
const PARTS =
  ".strip-thumb, .strip-face, .strip-card, .strip-caption, .strip-name, .strip-label, .strip-surface";
const partsOf = (s: HTMLElement) => [...s.querySelectorAll<HTMLElement>(PARTS)];
