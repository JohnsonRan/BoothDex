"use client";
// Avatar picker (Carousel Snap template block, CarouselSnap.tsx:22-27: a label above a snapping row of cards with
// prev/next buttons and the Carousel's own edge fade). Each card is Product Detail's selectable image thumbnail as
// is (a transparent SelectableCard filling a square AspectRatio; its corner radius and selection ring are the
// card's own, the image only fills and covers, inset by the ring's width so the ring shows); the name sits under it
// as the caption of the Top Nav template's category tiles. The toggle is Collapsible's trigger anatomy: a label and
// a chevron that turns up to fold. At full size it stands on the label line at the end opposite the label, clear of
// the cards and the Carousel's buttons, and the cards take the whole row; as they shrink it comes down to their
// middle at the row's end, and the cards' view is clipped short of it by its width and the row's gap
// (app/globals.css `strip-toggle`, `strip-clear`). Clipped, not narrowed, so nothing in the row moves.
// A mouse wheel over the strip scrolls it sideways while it can move that way, as on the old strip; the
// Carousel maps only Shift + wheel itself (Carousel.d.ts:5-8).
// The first page load renders only the cards a wide screen shows; the rest join on the first pointer, touch,
// focus or scroll in the strip, before any of them can come into view.
// The strip is one: under the filter band it shrinks with the page's scroll (band.tsx, app/globals.css
// `strip-*`), each card's square to 40 px about its bottom edge while its name stays as it is, and its label
// goes under the band. The cards are memoised: the toggle's state changes around them without drawing them again.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Button } from "@astryxdesign/core/Button";
import { Carousel, type CarouselHandle } from "@astryxdesign/core/Carousel";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Icon } from "@astryxdesign/core/Icon";
import { SelectableCard } from "@astryxdesign/core/SelectableCard";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import {
  type CSSProperties,
  memo,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useUi } from "@/components/providers";
import { type Avatar, avatarName, imageSrc } from "@/lib/catalog";
import { useFilterUrl } from "./filter-url";
import { SkeletonImage, THUMB_RADIUS } from "./silhouette";

// a card's square at full size and shrunk by the scroll, in rem (104 and 40 px at a 16 px root); Booth's avatar
// thumbnails are square. The band's shrink (band.tsx `--strip-d`, `--strip-scale`) is reckoned from these.
export const CARD_REM = 6.5;
export const SHRUNK_REM = 2.5;
const CARD_WIDTH = `${CARD_REM}rem`;
// cards rendered before the strip is used: more than fit across 1440 px (with the "all" card)
const FIRST_CARDS = 14;
const WAKE = ["pointerover", "touchstart", "focusin", "scroll"] as const;
// A touch screen swipes the strip, so it gets no prev/next buttons. They also break iOS Safari 26's toolbar: they
// sit in an always-open fixed top-layer popover, which Safari's toolbar sampling finds at the bottom edge
// (WebKit LocalFrameView::fixedContainerEdges), so it painted the toolbar solid instead of letting the page show
// through. Without buttons the Carousel hides that layer (display: none), which the sampling skips.
const TOUCH = "(hover: none) and (pointer: coarse)";
// the toggle's middle on the shrunk cards' middle: up from the row's bottom by a name line, the name's gap and half
// the difference between a shrunk card and the button
const TOGGLE_LIFT = `calc(var(--text-supporting-leading) * var(--text-supporting-size) + var(--spacing-2) + (${SHRUNK_REM}rem - var(--size-element-sm)) / 2)`;
// and from there up to the label line's middle at full size: a full card less half a shrunk one, the label's gap
// and half its line
const TOGGLE_UP = `calc(${CARD_REM - SHRUNK_REM / 2}rem + var(--spacing-3) + var(--text-body-leading) * var(--text-body-size) / 2)`;

// the toggle's two labels in one cell, the one not shown hidden: the button keeps its width as it turns, so a
// shorter label does not shift it along the row. Plain spans: a button holds phrasing content only (a Stack is a
// div), and Text would set its own type and colour over the button's
const LABELS: CSSProperties = {
  display: "inline-grid",
  justifyItems: "center",
};
const SHOWN: CSSProperties = { gridArea: "1 / 1" };
const UNSEEN: CSSProperties = { ...SHOWN, visibility: "hidden" };
const TURNS = ["strip.collapse", "strip.expand"] as const;
// the toggle over the row's end, out of its flow: the cards keep the whole row
const ROW: CSSProperties = { position: "relative" };
const TOGGLE = {
  position: "absolute",
  insetInlineEnd: 0,
  insetBlockEnd: TOGGLE_LIFT,
  "--strip-toggle-up": TOGGLE_UP,
} as CSSProperties;

const AllGlyph = () => (
  <VStack height="100%" hAlign="center" vAlign="center">
    <Icon icon="site:allAvatars" size="lg" color="secondary" />
  </VStack>
);

export function AvatarStrip({
  avatars,
  expanded,
  controls,
  onToggle,
  onPick,
}: {
  avatars: Avatar[];
  /** shown at full size: the toggle folds it */
  expanded: boolean;
  /** an id for the cards, which the toggle names as what it shows at full size or shrunk */
  controls: string;
  onToggle: () => void;
  onPick: () => void;
}) {
  const { t } = useUi();
  const turn = expanded ? "strip.collapse" : "strip.expand";
  const row = useRef<HTMLDivElement>(null);
  const toggle = useRef<HTMLButtonElement>(null);
  // the toggle's width, which the cards' clip clears once shrunk; its labels are the language's
  useLayoutEffect(() => {
    const r = row.current;
    const b = toggle.current;
    if (!r || !b) return;
    const sized = new ResizeObserver(() =>
      r.style.setProperty("--strip-toggle-w", `${b.offsetWidth}px`),
    );
    sized.observe(b);
    return () => sized.disconnect();
  }, []);
  return (
    <VStack gap={3}>
      <Text type="body" weight="bold" className="strip-label">
        {t("filter.avatar")}
      </Text>
      <HStack ref={row} className="strip-row" style={ROW}>
        <StackItem size="fill" id={controls} className="strip-cards">
          <Cards avatars={avatars} onPick={onPick} />
        </StackItem>
        <Button
          label={t(turn)}
          endContent={
            <Icon
              icon="chevronDown"
              style={expanded ? { transform: "rotate(180deg)" } : undefined}
            />
          }
          variant="ghost"
          size="sm"
          ref={toggle}
          className="strip-toggle"
          style={TOGGLE}
          aria-expanded={expanded}
          aria-controls={controls}
          onClick={onToggle}
        >
          <span style={LABELS}>
            {TURNS.map((k) => (
              <span key={k} style={k === turn ? SHOWN : UNSEEN}>
                {t(k)}
              </span>
            ))}
          </span>
        </Button>
      </HStack>
    </VStack>
  );
}

const Cards = memo(function Cards({
  avatars,
  onPick,
}: {
  avatars: Avatar[];
  onPick: () => void;
}) {
  const { t, ui } = useUi();
  const { filters, apply } = useFilterUrl();
  // the picked avatar shows as selected at once, before its results arrive
  const value = filters.avatar;
  const carousel = useRef<CarouselHandle>(null);
  const strip = useRef<HTMLDivElement>(null);
  const pick = (slug: string) => {
    onPick();
    apply({ avatar: slug });
  };
  const touch = useMediaQuery(TOUCH);

  // keep the chosen avatar in view, also when the page opens on one far down the list. The handle's scrollTo
  // brings a card to the start edge (CarouselHandle.scrollTo, Carousel.d.ts:41-46), where the "previous" button
  // covers it, so the card before it goes there and the chosen one sits whole beside the button.
  const index = avatars.findIndex((a) => a.slug === value) + 1;
  const [used, setUsed] = useState(false);
  const all = used || index >= FIRST_CARDS;
  useEffect(() => {
    if (index > 0) carousel.current?.scrollTo(index - 1);
  }, [index]);
  useEffect(() => {
    const el = strip.current;
    if (all || !el) return;
    const wake = () => setUsed(true);
    for (const e of WAKE)
      el.addEventListener(e, wake, {
        once: true,
        passive: true,
        capture: true,
      });
    return () => {
      for (const e of WAKE) el.removeEventListener(e, wake, { capture: true });
    };
  }, [all]);
  // non-passive, so the page stays put; trackpad sideways deltas and Shift + wheel stay the Carousel's. The
  // Carousel exposes no scroller, only its edge queries, so the scroll goes to its documented scroll-area
  // group, by the same scrollBy its Shift + wheel uses, which keeps the mandatory snap stepping a card at a time.
  useEffect(() => {
    const el = strip.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      const c = carousel.current;
      if (e.shiftKey || Math.abs(e.deltaY) <= Math.abs(e.deltaX) || !c) return;
      if (!(e.deltaY > 0 ? c.canScrollNext() : c.canScrollPrev())) return;
      e.preventDefault();
      el.querySelector(':scope > [role="group"]')?.scrollBy({
        left: e.deltaY,
        behavior: "auto",
      });
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  return (
    <Carousel
      gap={2}
      hasSnap
      hasButtons={!touch}
      handleRef={carousel}
      ref={strip}
      aria-label={t("strip.label")}
    >
      <Tile
        label={t("strip.all")}
        selected={!value}
        onSelect={() => pick("")}
        hasImage={false}
      >
        <AllGlyph />
      </Tile>
      {(all ? avatars : avatars.slice(0, FIRST_CARDS - 1)).map((a) => (
        <Tile
          key={a.slug}
          label={avatarName(a, ui)}
          selected={a.slug === value}
          onSelect={() => pick(a.slug === value ? "" : a.slug)}
          hasImage={!!a.image_url}
        >
          {a.image_url ? (
            <SkeletonImage
              radius={THUMB_RADIUS}
              src={imageSrc(a.image_url, "300x300_a2_g5")}
              loading="lazy"
              decoding="async"
            />
          ) : (
            <AllGlyph />
          )}
        </Tile>
      ))}
    </Carousel>
  );
});

function Tile({
  label,
  selected,
  onSelect,
  hasImage,
  children,
}: {
  label: string;
  selected: boolean;
  onSelect: () => void;
  /** a glyph instead of a picture sits on the muted card surface, so the card still reads as one */
  hasImage: boolean;
  children: React.ReactNode;
}) {
  return (
    <VStack gap={2} width={CARD_WIDTH}>
      <AspectRatio ratio={1} className="strip-thumb">
        <SelectableCard
          label={label}
          isSelected={selected}
          onChange={onSelect}
          variant={hasImage ? "transparent" : "muted"}
          padding={0.5}
          width="100%"
          height="100%"
        >
          {children}
        </SelectableCard>
      </AspectRatio>
      <Text
        type="supporting"
        maxLines={1}
        justify="center"
        hasTruncateTooltip={false}
      >
        {label}
      </Text>
    </VStack>
  );
}
