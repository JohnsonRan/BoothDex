"use client";
// The filter band: the toolbar row pinned under the top bar while the window scrolls, as a sticky child of a
// LayoutContent that does not scroll itself (LayoutContent.d.ts:28-33, its example :70-75; the style object as
// product-detail's sticky column, page.tsx:26-30, opaque as dashboard-alert-rail's sticky header, :895-900). It
// carries the page header's divider and, on that edge, the refresh progress (refresh.tsx).
// The avatar strip is the band's lower part: the band sticks a strip's shrink higher (`--strip-d`) with the toolbar
// pinned inside it, so once the toolbar reaches the top bar the strip goes on under it for that far while its
// cards morph into a row of chips the height of the toolbar's controls (avatar-strip.tsx), then stays, laid over
// the results that scroll beneath. The morph is scroll-driven CSS (app/strip.css `strip-*`, transform and
// opacity only); nothing in the flow changes, so nothing below moves. The band tells the strip where the scroll
// has it, so shrunk and in use it lays its chips out for real.
// The toggle in the toolbar's end cluster (strip-toggle.tsx, handed the strip's state here) opens the shrunk strip
// over the results (the band sticks at the top bar's edge, the strip's surface whole under it, and the strip runs
// the morph back to full size over time, lib/motion `turnAlong`) and folds it back the same way, laid out open
// until the morph ends. At full size in its place it folds the strip there along the morph (`tucked`): only the
// chip row stays in the flow, the results slide up, the band has no shrink
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
import type { Facets } from "@/components/catalog/filter/filter-style";
import { RefreshProgress } from "@/components/catalog/refresh";
import type { Avatar } from "@/lib/catalog";
import {
  cancelOnReducedMotion,
  morphsOf,
  prefersReducedMotion,
  turnAlong,
} from "@/lib/motion";
import {
  AvatarStrip,
  CARD_REM,
  CHIP_REM,
  type StripMorph,
} from "./avatar-strip";
import { attachBand, attachBar } from "./band-edge";
import { StripToggleContext, type StripToggleState } from "./strip-toggle";

// the top bar's height, which the AppShell keeps on its root for its own sticky parts
const TOP = "var(--_app-shell-header-height, 0px)";
// The band reaches up `--strip-d` (app/strip.css) past the top bar, under it: only its toolbar and its strip
// paint and take the pointer, each where it shows.
const HEAD: CSSProperties = {
  position: "sticky",
  insetBlockStart: `calc(${TOP} - var(--strip-d))`,
  zIndex: 1,
  pointerEvents: "none",
  // the strip turning over the results slides up past the band's top, where its opaque surface would paint over the
  // top bar (the same z-index, earlier in the DOM); clipped at the top edge only, the other sides stay open
  clipPath: "inset(0 -100vw -100vh)",
};
const OPEN: CSSProperties = { ...HEAD, insetBlockStart: TOP };
const BAR: CSSProperties = {
  position: "sticky",
  insetBlockStart: TOP,
  zIndex: 1,
  pointerEvents: "auto",
  backgroundColor: "var(--color-background-surface)",
  // none beside the filters' rail over the avatar strip, whose own edge is the band's there (app/filter-row.css)
  borderBlockEnd: "var(--band-edge, 1px solid var(--color-border))",
};
// its own layer of the band, so the surface behind it stays behind its cards
const STRIP: CSSProperties = { position: "relative", isolation: "isolate" };
// A card's top to its chip's: a chip ends where its card's caption does (the card, the caption's gap and line).
const STRIP_DY = `calc(${CARD_REM - CHIP_REM}rem + var(--spacing-2) + var(--text-supporting-leading) * var(--text-supporting-size))`;
// How far the strip goes under the toolbar: its top padding, its label line and the label's gap, then a card's
// top to its chip's less the 8 px a chip keeps from the toolbar, as many as the strip keeps below it. Registered
// as a length (app/strip.css), so it reads back in px.
const STRIP_D = `calc(var(--spacing-4) + var(--text-body-leading) * var(--text-body-size) + var(--spacing-3) + ${STRIP_DY} - var(--spacing-2))`;
// the chip's scale of the card (app/strip.css `strip-thumb`)
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
  facets,
  folded: foldedNow = false,
  children,
}: {
  label: string;
  /** the filter row and the end cluster; none in the recent view */
  toolbar?: ReactNode;
  /** the avatar strip's avatars, in the items view */
  avatars?: Avatar[];
  /** the search's reading of query models; the strip stays available while it loads */
  facets: Promise<Facets>;
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
  // what the hand made of the strip: nothing (the scroll has it), opened over the results, folding back over the
  // results (laid out open until the morph ends), or folded in place (shrunk, out of the scroll's hands)
  const [hand, setHand] = useState<"none" | "open" | "closing" | "tucked">(
    "none",
  );
  // the effects key on these two, not on `hand`: the end of a fold back over the results reruns none of them
  const open = hand === "open";
  const tucked = hand === "tucked";
  const laidOpen = open || hand === "closing";
  // the scroll the shrink starts at and its length
  const span = useRef({ start: 0, length: 0, shrunk: folded });
  const openNow = useRef(open);
  openNow.current = open;
  const morph = useRef<StripMorph>(null);
  // where the strip stood before a hand turn (and its morphs, when the turn's layout drops them), and that turn
  // while it runs: the scroll's marks wait for its end
  const placed = useRef<{
    top: number;
    morphs?: ReturnType<typeof morphsOf>;
  } | null>(null);
  const turning = useRef<Animation[] | null>(null);
  // the morphs the strip opened over the results along, to fold back along: open, its parts run none
  const opened = useRef<ReturnType<typeof morphsOf>>(new Map());
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
      // Carousel's buttons and, shrunk, draws the strip as the morph ends (app/strip.css); a hand turn is
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
          : p >= 1 && !openNow.current && !turning.current
            ? "rest"
            : "part",
      );
      // back at full size with the page, an opened strip is the same as one that is not
      if (p <= 0) setHand((h) => (h === "open" ? "none" : h));
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
    rerun.current = run;
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

  // A hand turn: the scroll's marks wait for its end, where `done` lets the layout take over before the turn
  // lets go.
  const play = useCallback((running: Animation[], done: () => void) => {
    turning.current = running;
    rerun.current();
    Promise.allSettled(running.map((a) => a.finished)).then(() => {
      if (turning.current !== running) return;
      done();
      for (const a of running) a.cancel();
      turning.current = null;
      rerun.current();
    });
  }, []);

  // opened over the results, or folded or grown back in place, by hand: the strip runs the scroll's morph from where
  // it stood, in place with the results. Folded in place, the strip is at rest once the morph has drawn it, and in
  // use: the toggle was pressed.
  useLayoutEffect(() => {
    const from = placed.current;
    const s = stripRef.current;
    const results = resultsRef.current;
    placed.current = null;
    if (!from || !s || !results) return;
    const running = turnAlong(
      open ? [s] : [s, results],
      [from.top - s.getBoundingClientRect().top, 0],
      from.morphs ?? morphsOf(partsOf(s)),
      tucked,
    );
    play(running, () => {
      if (!tucked) return;
      morph.current?.settle("rest");
      morph.current?.use();
    });
    return cancelOnReducedMotion(running);
  }, [open, tucked, play]);

  const turn = useCallback(
    (next: boolean) => {
      const s = stripRef.current;
      if (!s) return;
      if (next) {
        // opened from the chips: the cards drawn as the chips are, which the morph takes to full size over the
        // whole surface (`data-strip-open`), so the surface's own fade stays out
        morph.current?.settle("part");
        opened.current = morphsOf(
          partsOf(s).filter((el) => !el.matches(".strip-surface")),
        );
        placed.current = {
          top: s.getBoundingClientRect().top,
          morphs: opened.current,
        };
        setHand("open");
      } else if (span.current.shrunk) {
        setHand("closing");
        play(
          turnAlong([s], [0, -span.current.length], opened.current, true),
          () => flushSync(() => setHand("none")),
        );
      } else setHand("none");
    },
    [play],
  );
  const tuck = useCallback((next: boolean) => {
    const s = stripRef.current;
    // folded, the parts drop their morphs: taken now
    if (s)
      placed.current = {
        top: s.getBoundingClientRect().top,
        morphs: next ? morphsOf(partsOf(s)) : undefined,
      };
    setHand(next ? "tucked" : "none");
  }, []);
  const toggle = useCallback(() => {
    if (turning.current) return;
    if (open) turn(false);
    else if (tucked) {
      // the chips go back to the cards' row, drawn as they are; at the top the strip's place is read first, before
      // the cards' layout change would make that read lay the strip out
      if (scrollY <= span.current.start) {
        tuck(false);
        morph.current?.settle("part");
      } else {
        morph.current?.settle("part");
        // scrolled past: the shrink goes back in the flow above the tiles and the window as far down, in one frame
        const results = resultsRef.current;
        const was = results?.getBoundingClientRect().top ?? 0;
        flushSync(() => setHand("none"));
        scrollBy({
          top: (results?.getBoundingClientRect().top ?? 0) - was,
          behavior: "instant",
        });
        turn(true);
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
    if (openNow.current && !turning.current) turn(false);
  }, [turn]);

  const strip = useMemo(
    () =>
      shown && (
        <AvatarStrip
          avatars={shown}
          facets={facets}
          controls={id}
          onPick={picked}
          folded={folded}
          morph={morph}
        />
      ),
    [shown, facets, id, picked, folded],
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
        data-strip-open={laidOpen ? "" : undefined}
        data-strip-tucked={tucked ? "" : undefined}
        // a start past page 1 is shrunk from the first paint; page 1 shrinks once measured
        data-morph={avatars && folded ? "" : undefined}
        data-shrink={avatars && folded ? "all" : undefined}
        style={
          avatars
            ? ({
                ...(laidOpen ? OPEN : HEAD),
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

/** what the scroll morphs or fades in the strip (app/strip.css), each run by a hand turn */
const PARTS =
  ".strip-thumb, .strip-face, .strip-card, .strip-caption, .strip-name, .strip-label, .strip-surface";
const partsOf = (s: HTMLElement) => [...s.querySelectorAll<HTMLElement>(PARTS)];
