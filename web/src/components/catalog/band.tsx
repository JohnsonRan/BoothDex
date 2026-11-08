"use client";
// The filter band: the toolbar row pinned under the top bar while the window scrolls, as a sticky child of a
// LayoutContent that does not scroll itself (LayoutContent.d.ts:28-33, its example :70-75; the style object as
// product-detail's sticky column, page.tsx:26-30, opaque as dashboard-alert-rail's sticky header, :895-900). It
// carries the page header's divider and, on that edge, the refresh progress (refresh.tsx).
// The avatar strip sits under it at full size and scrolls away beneath it. Once it is wholly under the band, a
// compact row docks along the band's bottom edge, laid over what scrolls beneath, so the band's own height and
// everything below it stay where they are; the row leaves again once the strip comes out from under it (the row's
// height apart, so the two never trade places back and forth). The strip's toggle folds it by scrolling it under the band, or, where the page is too short for
// that, by closing its box; the row's toggle opens the strip again, as a panel over the row while the page is
// scrolled. The row and the panel are drawn from the start, hidden until wanted: showing one is a fade, not a new
// strip. A start past page 1 renders with the row docked: the page opens scrolled to its results
// (deep-start.ts).
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
  dock,
  prefersReducedMotion,
  shut,
} from "@/lib/motion";
import { AvatarStrip } from "./avatar-strip";
import { RefreshProgress } from "./refresh";

const SURFACE: CSSProperties = {
  backgroundColor: "var(--color-background-surface)",
  borderBlockEnd: "1px solid var(--color-border)",
};
const STUCK: CSSProperties = {
  ...SURFACE,
  position: "sticky",
  // the top bar's height, which the AppShell keeps on its root for its own sticky parts
  insetBlockStart: "var(--_app-shell-header-height, 0px)",
  zIndex: 1,
};
// under the toolbar at the toolbar row's gap (4, 16 px): up over the band's bottom padding (6) by the difference,
// and over the band's divider, which the row carries on instead
const ROW: CSSProperties = {
  ...SURFACE,
  position: "absolute",
  insetBlockStart: "calc(100% - var(--spacing-2))",
  insetInline: 0,
};
// over the row, which keeps its place, and down over the results
const PANEL: CSSProperties = {
  ...SURFACE,
  position: "absolute",
  insetBlockStart: 0,
  insetInline: 0,
};
// the content's gap, which a closed strip gives up with its height
const GAP = "var(--spacing-4)";
const CLOSED: CSSProperties = {
  blockSize: 0,
  marginBlockEnd: `calc(-1 * ${GAP})`,
  overflow: "clip",
  visibility: "hidden",
};

let band: HTMLElement | null = null;
let row: HTMLElement | null = null;

/**
 * The viewport px above the line the band's bottom edge holds once stuck, the compact row's included wherever the
 * strip is (shown or not, so a place saved and the same place restored agree): content above it is under the top
 * bar or the band. Scrolling does not change it, so a place measured from it is found again at the same scroll.
 * deep-start.ts reads the same line before the page hydrates.
 */
export function covered() {
  if (!band) return 0;
  const b = band.getBoundingClientRect();
  return (
    Number.parseFloat(getComputedStyle(band).top) +
    (row ? row.getBoundingClientRect().bottom : b.bottom) -
    b.top
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
  /** the page opens scrolled past the strip (a start past page 1): the row is docked from the first paint */
  folded?: boolean;
  /** the results */
  children: ReactNode;
}) {
  const ref = useRef<HTMLElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const slot = useRef<HTMLDivElement>(null);
  const rowRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const id = useId();
  // the strip is under the band: the row stands in for it
  const [under, setUnder] = useState(folded);
  // folded by hand where the page could not scroll it away
  const [closed, setClosed] = useState(false);
  const [panel, setPanel] = useState(false);
  const docked = !!avatars && (under || closed);
  const state = useRef({ closed });
  state.current.closed = closed;

  useLayoutEffect(() => {
    band = ref.current;
    row = rowRef.current;
    return () => {
      band = null;
      row = null;
    };
  }, []);

  useEffect(() => {
    if (!avatars || closed) return;
    let frame = 0;
    const run = () => {
      frame = 0;
      const s = slot.current?.getBoundingClientRect();
      const b = ref.current?.getBoundingClientRect();
      const r = rowRef.current?.getBoundingClientRect();
      if (!s || !b || !r) return;
      // in under the band's edge, out once clear of the row (half a px for a deep start's rounding)
      const next = under ? s.bottom <= r.bottom + 0.5 : s.bottom <= b.bottom;
      if (next === under) return;
      if (!next) setPanel(false);
      setUnder(next);
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(run);
    };
    schedule();
    addEventListener("scroll", schedule, { passive: true });
    // the pages above a deep start go in after the band hydrates, and WebKit sends no scroll for the jump
    const grown = new ResizeObserver(schedule);
    if (content.current) grown.observe(content.current);
    return () => {
      cancelAnimationFrame(frame);
      removeEventListener("scroll", schedule);
      grown.disconnect();
    };
  }, [avatars, closed, under]);

  // the row, the panel and a closed strip glide; the first render is the server's and stays as it is
  const was = useRef({ docked, panel, closed });
  useLayoutEffect(() => {
    const w = was.current;
    was.current = { docked, panel, closed };
    const running: Animation[] = [];
    if (w.docked !== docked && rowRef.current)
      running.push(...dock(rowRef.current, docked));
    if (w.panel !== panel && panelRef.current) {
      running.push(...dock(panelRef.current, panel));
      // focus in the closing panel goes back to the row's toggle
      if (!panel && panelRef.current.contains(document.activeElement))
        rowRef.current?.querySelector<HTMLElement>("[aria-expanded]")?.focus();
    }
    const s = slot.current;
    if (w.closed !== closed && s)
      running.push(...shut(s, closed, s.scrollHeight, GAP));
    return running.length ? cancelOnReducedMotion(running) : undefined;
  }, [docked, panel, closed]);

  // Folds by hand: scrolls the strip under the band, where it folds itself; a page too short for that closes it.
  const fold = useCallback(() => {
    const s = slot.current?.getBoundingClientRect();
    const b = ref.current;
    if (!s || !b) return;
    // to the edge the band holds once stuck, a px past it: a smooth scroll may stop a fraction short
    const edge =
      Number.parseFloat(getComputedStyle(b).top) +
      b.getBoundingClientRect().height;
    const need = Math.ceil(s.bottom - edge) + 1;
    const room = document.documentElement.scrollHeight - innerHeight - scrollY;
    if (need <= room)
      scrollBy({
        top: need,
        behavior: prefersReducedMotion() ? "instant" : "smooth",
      });
    else setClosed(true);
  }, []);
  // Unfolds by hand: a closed strip in view opens in place, else the panel shows it over the row.
  const unfold = useCallback(() => {
    const s = slot.current?.getBoundingClientRect();
    const b = ref.current?.getBoundingClientRect();
    if (state.current.closed && s && b && s.top >= b.bottom) setClosed(false);
    else setPanel(true);
  }, []);
  const hide = useCallback(() => setPanel(false), []);

  // made once, so a fold draws none of them again (AvatarStrip is memoised)
  const strips = useMemo(
    () =>
      avatars && {
        full: (
          <AvatarStrip
            avatars={avatars}
            compact={false}
            controls={id}
            onToggle={fold}
            onPick={hide}
          />
        ),
        compact: (
          <AvatarStrip
            avatars={avatars}
            compact
            controls={id}
            onToggle={unfold}
            onPick={hide}
          />
        ),
        panel: (
          <AvatarStrip
            avatars={avatars}
            compact={false}
            controls={id}
            onToggle={hide}
            onPick={hide}
          />
        ),
      },
    [avatars, id, fold, unfold, hide],
  );

  return (
    <VStack gap={0}>
      <Section
        ref={ref}
        aria-label={label}
        variant="transparent"
        padding={toolbar ? 6 : 0}
        paddingBlockStart={toolbar ? 4 : 0}
        className="filter-toolbar"
        style={STUCK}
      >
        <VStack gap={4}>{toolbar}</VStack>
        {!docked && <RefreshProgress />}
        {strips && (
          // hidden is out of reach too: no focus, no pointer, nothing read
          <VStack
            ref={rowRef}
            data-strip-row=""
            style={docked ? ROW : { ...ROW, visibility: "hidden" }}
          >
            {/* the row keeps its place under the panel, which shows the strip in it: each avatar once */}
            <VStack
              paddingInline={6}
              paddingBlockEnd={6}
              style={panel ? { visibility: "hidden" } : undefined}
            >
              {strips.compact}
            </VStack>
            {docked && <RefreshProgress />}
            {/* a VStack's padding, not a Section's: a Carousel bleeds into a Section's inset */}
            <VStack
              ref={panelRef}
              id={panel ? id : undefined}
              style={panel ? PANEL : { ...PANEL, visibility: "hidden" }}
            >
              <VStack paddingInline={6} paddingBlockEnd={6}>
                {strips.panel}
              </VStack>
            </VStack>
          </VStack>
        )}
      </Section>
      <VStack
        ref={content}
        data-band-content=""
        gap={4}
        padding={6}
        paddingBlockStart={4}
        // the results keep their own place through every change (result-pages.tsx), in every engine alike
        style={{ overflowAnchor: "none" }}
      >
        {strips && (
          <VStack
            ref={slot}
            data-strip=""
            id={panel ? undefined : id}
            style={closed ? CLOSED : undefined}
          >
            {strips.full}
          </VStack>
        )}
        {children}
      </VStack>
    </VStack>
  );
}
