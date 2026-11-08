"use client";
// The filter band: the toolbar row pinned under the top bar while the window scrolls, as a sticky child of a
// LayoutContent that does not scroll itself (LayoutContent.d.ts:28-33, its example :70-75; the style object as
// product-detail's sticky column, page.tsx:26-30, opaque as dashboard-alert-rail's sticky header, :895-900). It
// carries the page header's divider and, on that edge, the refresh progress (refresh.tsx).
// The avatar strip sits under it at full size and scrolls away beneath it; once it is wholly under the band, a
// compact row takes its place in the band, an empty box of its height takes the strip's, and the results draw up
// under the band by what the row added to it, so nothing below the band moves (not even the results' own box,
// which a layout shift would count). The strip's chevron folds it into that row by hand, and the row's chevron unfolds it: in
// place while the strip's place is in view, else as a panel over the results. A start past page 1 begins folded.
// The state is the page's own, as the Collapsible blocks' custom disclosure (CollapsibleHookUsage.tsx:14-42: the
// open state, a trigger with aria-expanded and aria-controls, the content it shows).

import { Section } from "@astryxdesign/core/Section";
import { VStack } from "@astryxdesign/core/Stack";
import {
  type CSSProperties,
  type ReactNode,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import type { Avatar } from "@/lib/catalog";
import { cancelOnReducedMotion, swapIn } from "@/lib/motion";
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
const PANEL: CSSProperties = {
  ...SURFACE,
  position: "absolute",
  insetBlockStart: "100%",
  insetInline: 0,
};

let band: HTMLElement | null = null;
const height = (el: Element) => el.getBoundingClientRect().height;

/**
 * The viewport px above the line the band's bottom edge holds once stuck: content above it is under the top bar
 * or the band. Scrolling does not change it, so a place measured from it is found again at the same scroll.
 */
export function covered() {
  if (!band) return 0;
  return (
    Number.parseFloat(getComputedStyle(band).top) +
    band.getBoundingClientRect().height
  );
}

export function FilterBand({
  label,
  toolbar,
  avatars,
  page,
  children,
}: {
  label: string;
  /** the filter row and the end cluster; none in the recent view */
  toolbar?: ReactNode;
  /** the avatar strip's avatars, in the items view */
  avatars?: Avatar[];
  /** the page the server rendered */
  page: number;
  /** the results */
  children: ReactNode;
}) {
  const ref = useRef<HTMLElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const slot = useRef<HTMLDivElement>(null);
  const row = useRef<HTMLDivElement>(null);
  const id = useId();
  const [open, setOpen] = useState(page === 1);
  // the open strip is wholly under the band: the compact row stands in for it
  const [under, setUnder] = useState(false);
  const [panel, setPanel] = useState(false);
  // the band's growth by the compact row while the strip is under it
  const [lift, setLift] = useState(0);
  // the strip's and the band's heights when the strip went under
  const was = useRef({ strip: 0, band: 0 });

  useLayoutEffect(() => {
    band = ref.current;
    return () => {
      band = null;
    };
  }, []);

  useEffect(() => {
    if (!avatars || !open) return;
    let frame = 0;
    const run = () => {
      frame = 0;
      const b = ref.current;
      const s = slot.current;
      if (!b || !s) return;
      if (
        s.getBoundingClientRect().bottom <= b.getBoundingClientRect().bottom ===
        under
      )
        return;
      if (!under) was.current = { strip: height(s), band: height(b) };
      else setPanel(false);
      setUnder(!under);
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(run);
    };
    schedule();
    addEventListener("scroll", schedule, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      removeEventListener("scroll", schedule);
    };
  }, [avatars, open, under]);

  // before the change paints: the results draw up by what the row added to the band, and what came in fades in
  useLayoutEffect(() => {
    const b = ref.current;
    if (b) setLift(under ? height(b) - was.current.band : 0);
    const el = under ? row.current : slot.current;
    if (el && was.current.strip) return cancelOnReducedMotion(swapIn(el));
  }, [under]);

  const unfold = () => {
    const top = content.current?.getBoundingClientRect().top ?? 0;
    if (!open && top >= (ref.current?.getBoundingClientRect().bottom ?? 0))
      setOpen(true);
    else setPanel(!panel);
  };
  const strip = (compact: boolean, onToggle: () => void) =>
    avatars && (
      <AvatarStrip
        avatars={avatars}
        compact={compact}
        controls={id}
        onToggle={onToggle}
        onPick={() => setPanel(false)}
      />
    );
  const docked = !!avatars && (!open || under);

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
        <VStack gap={4}>
          {toolbar}
          {docked && <VStack ref={row}>{strip(true, unfold)}</VStack>}
        </VStack>
        {docked && panel && (
          // a VStack's padding, not a Section's: a Carousel bleeds into a Section's inset
          <VStack id={id} padding={6} paddingBlockStart={4} style={PANEL}>
            {strip(false, () => setPanel(false))}
          </VStack>
        )}
        <RefreshProgress />
      </Section>
      <VStack
        ref={content}
        gap={4}
        padding={6}
        paddingBlockStart={4}
        style={{ marginBlockStart: -lift }}
      >
        {avatars && open && (
          <VStack ref={slot} id={under ? undefined : id}>
            {under ? (
              <VStack aria-hidden style={{ blockSize: was.current.strip }} />
            ) : (
              strip(false, () => setOpen(false))
            )}
          </VStack>
        )}
        {children}
      </VStack>
    </VStack>
  );
}
