"use client";
// Avatar picker (Carousel Snap template block, CarouselSnap.tsx:22-27: a label above a snapping row of cards with
// prev/next buttons and the Carousel's own edge fade). Each card is Product Detail's selectable image thumbnail as
// is (a transparent SelectableCard filling a square AspectRatio; its corner radius and selection ring are the
// card's own, the image only fills and covers, inset past the ring so the ring shows on any image); the name sits
// under it as the caption of the Top Nav template's category tiles. Its toggle is in the filter toolbar
// (strip-toggle.tsx), so the cards take the whole row.
// A mouse wheel over the strip scrolls it sideways while it can move that way, as on the old strip; the
// Carousel maps only Shift + wheel itself (Carousel.d.ts:5-8).
// The first page load renders only the cards a wide screen shows; the rest join on the first pointer, touch,
// focus or scroll in the strip, before any of them can come into view, and with a mouse also once the page is idle,
// so a screen wider than the first cards fills in unasked. A touch screen shows a few cards and swipes the rest in,
// its touch mounting them before they move: it is spared rendering them all at load.
// The strip is one: under the filter band it shrinks with the page's scroll (band.tsx, app/strip.css `strip-*`)
// into a row of chips, each the card's own image round at Avatar's 32 px size with its name beside it, as the
// table-filter template's person cell (page.tsx:2862-2867: an Avatar and its name in an HStack, gap 2, centred).
// Each card's square moves and shrinks to its chip's place, measured here once (and again on a resize, a font load
// or a sideways scroll of the cards) as a distance per card (`--chip-x`) and one for the row (`--chip-ax`), which
// brings the first card in view to the row's start; over it the same image fades in clipped round (AspectRatio
// `shape="ellipse"`, AspectRatio.tsx:68-78), the caption fades out and the chip's name in, the selected one
// semibold on a filled capsule with no ring (app/strip.css). All of it is transform and
// opacity, so the compositor runs it with the scroll. Shrunk and in use (a pointer, a touch, focus or a wheel on
// it, or the toggle folding it), the chips are laid out for real (`data-chips`), as they were drawn, so the row scrolls and snaps chip by chip. The cards are memoised: the band's state changes around them
// without drawing them again.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Carousel, type CarouselHandle } from "@astryxdesign/core/Carousel";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Icon } from "@astryxdesign/core/Icon";
import { SelectableCard } from "@astryxdesign/core/SelectableCard";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import {
  type CSSProperties,
  memo,
  type RefObject,
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import {
  type Facets,
  useFacets,
} from "@/components/catalog/filter/filter-style";
import { useFilterUrl } from "@/components/catalog/filter/filter-url";
import {
  SkeletonImage,
  THUMB_PADDING,
  THUMB_RADIUS,
} from "@/components/catalog/silhouette";
import { useUi } from "@/components/providers";
import {
  type Avatar,
  avatarName,
  imageSrc,
  selectAvatar,
  selectedAvatars,
} from "@/lib/catalog";

// a card's square at full size and a chip's round avatar (Avatar's 32 px size), in rem at a 16 px root; Booth's
// avatar thumbnails are square. The band's shrink (band.tsx `--strip-d`, `--strip-scale`) is reckoned from these.
export const CARD_REM = 6.5;
export const CHIP_REM = 2;
// A chip's name with the space after it at most this wide, ellipsised past it; the "all" glyph (Icon lg, 1.5rem)
// at 1rem in its chip, drawn that much larger on the round face the card shrinks with.
const SIZES = {
  "--strip-card": `${CARD_REM}rem`,
  "--strip-chip": `${CHIP_REM}rem`,
  "--strip-name": "8rem",
  "--strip-glyph": 1 / (1.5 * (CHIP_REM / CARD_REM)),
} as CSSProperties;
// cards rendered before the strip is used: more than fit across 2048 px (with the "all" card)
const FIRST_CARDS = 20;
const WAKE = ["pointerover", "touchstart", "focusin", "scroll"] as const;
// what puts the shrunk strip to use: its chips are then laid out for real
const USE = ["pointerover", "touchstart", "focusin", "wheel"] as const;
// A touch screen swipes the strip, so it gets no prev/next buttons. They also break iOS Safari 26's toolbar: they
// sit in an always-open fixed top-layer popover, which Safari's toolbar sampling finds at the bottom edge
// (WebKit LocalFrameView::fixedContainerEdges), so it painted the toolbar solid instead of letting the page show
// through. Without buttons the Carousel hides that layer (display: none), which the sampling skips.
const TOUCH = "(hover: none) and (pointer: coarse)";

/** Where the page's scroll has the strip (band.tsx): full size, shrinking, or shrunk and at rest. */
type StripState = "none" | "part" | "rest";
export type StripMorph = {
  settle: (state: StripState) => void;
  /** The strip is in use: shrunk and at rest, its chips are laid out for real. */
  use: () => void;
};

// The Carousel's scroll area, its documented scroll-area group; it exposes no scroller, only its edge queries.
const scrollArea = (strip: HTMLElement) =>
  strip.querySelector<HTMLElement>(':scope > [role="group"]');

// The chip row the scroll morphs the cards into: each chip's start in it (its card's in the cards' row is the card
// pitch times its index), the row's length, the row's distance (`--chip-ax`), and where the strip stands.
type Row = {
  chipX: number[];
  length: number;
  pitch: number;
  ax: number;
  state: StripState;
  /** the chips laid out for real */
  chips: boolean;
};

const shift = (el: HTMLElement, r: Row, ax: number) => {
  r.ax = ax;
  el.style.setProperty("--chip-ax", `${ax}px`);
};

// The row's distance: the card nearest the row's start becomes the first chip, or the chips end at the strip's end.
// Taken as the cards stand, before they move.
function anchor(el: HTMLElement, r: Row) {
  const area = scrollArea(el);
  if (!area || !r.pitch || r.chips) return;
  const s = area.scrollLeft;
  const first = Math.min(r.chipX.length - 1, Math.round(s / r.pitch));
  const end = Math.max(0, r.length - area.clientWidth);
  const ax = s - Math.min(r.chipX[first] ?? 0, end);
  if (ax !== r.ax) shift(el, r, ax);
}

// Each chip's width is its name's start and width (app/strip.css `.strip-name`), as wide as the tile laid out as
// a chip; each card's distance to its chip (`--chip-x`) follows.
function measure(el: HTMLElement | null, r: Row) {
  const area = el && scrollArea(el);
  const tiles = area
    ? [...area.querySelectorAll<HTMLElement>(".strip-tile")]
    : [];
  const thumb = tiles[0]?.querySelector<HTMLElement>(".strip-thumb");
  if (!el || !area || !thumb) return;
  // read everything, then write: a write between reads would make each read lay the strip out again
  const gap = Number.parseFloat(getComputedStyle(area).columnGap) || 0;
  const pitch = Number.parseFloat(getComputedStyle(thumb).width) + gap;
  const names = tiles.map((t) => t.querySelector<HTMLElement>(".strip-name"));
  const lead = names[0]
    ? Number.parseFloat(getComputedStyle(names[0]).insetInlineStart)
    : 0;
  const widths = names.map(
    (n) => lead + (n?.getBoundingClientRect().width ?? 0),
  );
  const chipX: number[] = [];
  let x = 0;
  for (const w of widths) {
    chipX.push(x);
    x += w + gap;
  }
  for (const [i, t] of tiles.entries())
    t.style.setProperty("--chip-x", `${chipX[i] - i * pitch}px`);
  Object.assign(r, { chipX, length: x - gap, pitch });
  anchor(el, r);
}

// Scrolls the row to a chip's or a card's start at once. Chromium's mandatory snap may pick another start for a
// scroll made as the layout changes, so the snap is off for it; back on, it finds the row on a start already.
function jump(area: HTMLElement, left: number) {
  area.style.scrollSnapType = "none";
  area.scrollTo({ left, behavior: "instant" });
  area.style.scrollSnapType = "";
}

// Shrunk to the end and in use, the chips are laid out for real, the row scrolled to where they were drawn; as the
// strip leaves rest, the cards are drawn as the chips were, the chip nearest the row's start the first card in view.
// The chips wait for use, so a scroll through the shrink never lays them out; leaving rest undoes it once.
function lay(el: HTMLElement | null, r: Row, chips: boolean) {
  const area = el && scrollArea(el);
  if (chips === r.chips || !el || !area) return;
  const s = area.scrollLeft;
  r.chips = chips;
  if (chips) {
    el.dataset.chips = "";
    jump(area, s - r.ax);
    return;
  }
  let first = 0;
  for (const [i, x] of r.chipX.entries())
    if (Math.abs(x - s) < Math.abs(r.chipX[first] - s)) first = i;
  delete el.dataset.chips;
  jump(area, first * r.pitch);
  shift(el, r, area.scrollLeft - s);
}

const AllGlyph = () => (
  <VStack height="100%" hAlign="center" vAlign="center" className="strip-glyph">
    <Icon icon="site:allAvatars" size="lg" color="secondary" />
  </VStack>
);

export function AvatarStrip({
  avatars,
  facets,
  controls,
  onPick,
  folded,
  morph,
}: {
  avatars: Avatar[];
  facets: Promise<Facets>;
  /** an id for the cards, which the toolbar's toggle names as what it shows at full size or shrunk */
  controls: string;
  onPick: () => void;
  /** shrunk from the first paint (band.tsx): the chips are laid out from the server on */
  folded: boolean;
  /** how the band tells the cards where the scroll has the strip */
  morph: RefObject<StripMorph | null>;
}) {
  const { t } = useUi();
  return (
    <VStack gap={3}>
      <Text type="body" weight="bold" className="strip-label">
        {t("filter.avatar")}
      </Text>
      <HStack className="strip-row">
        <StackItem size="fill" id={controls}>
          <Cards
            avatars={avatars}
            facets={facets}
            onPick={onPick}
            folded={folded}
            morph={morph}
          />
        </StackItem>
      </HStack>
    </VStack>
  );
}

const Cards = memo(function Cards({
  avatars,
  facets,
  onPick,
  folded,
  morph,
}: {
  avatars: Avatar[];
  facets: Promise<Facets>;
  onPick: () => void;
  folded: boolean;
  morph: RefObject<StripMorph | null>;
}) {
  const { t, ui } = useUi();
  const { filters, apply } = useFilterUrl();
  const answered = useFacets(facets);
  // Never use an earlier query's reading while the next search is still loading.
  const marks = answered?.query === filters.q ? answered.marks : [];
  // A typed model is selected too; an explicit pick still shows before its results arrive.
  const selected = selectedAvatars(filters.avatar, marks);
  const carousel = useRef<CarouselHandle>(null);
  const strip = useRef<HTMLDivElement>(null);
  const pick = (slug: string) => {
    onPick();
    apply(selectAvatar(filters.q, slug, marks));
  };
  const touch = useMediaQuery(TOUCH);

  // keep the chosen avatar in view, also when the page opens on one far down the list. The handle's scrollTo
  // brings a card to the start edge (CarouselHandle.scrollTo, Carousel.d.ts:41-46), where the "previous" button
  // covers it, so the card before it goes there and the chosen one sits whole beside the button.
  const index = avatars.findIndex((a) => selected.has(a.slug)) + 1;
  const [used, setUsed] = useState(false);
  const all = used || index >= FIRST_CARDS;
  useEffect(() => {
    if (index > 0) carousel.current?.scrollTo(index - 1);
  }, [index]);
  useEffect(() => {
    const el = strip.current;
    if (all || !el) return;
    const wake = () => setUsed(true);
    // read when it runs: the hook's first value is the server's, which is not a touch screen
    const idle = () => {
      if (!matchMedia(TOUCH).matches) wake();
    };
    const later =
      typeof requestIdleCallback === "function"
        ? requestIdleCallback(idle, { timeout: 2000 })
        : window.setTimeout(idle, 500);
    for (const e of WAKE)
      el.addEventListener(e, wake, {
        once: true,
        passive: true,
        capture: true,
      });
    return () => {
      if (typeof cancelIdleCallback === "function") cancelIdleCallback(later);
      else clearTimeout(later);
      for (const e of WAKE) el.removeEventListener(e, wake, { capture: true });
    };
  }, [all]);
  // non-passive, so the page stays put; trackpad sideways deltas and Shift + wheel stay the Carousel's. The
  // scroll goes to the Carousel's scroll area by the same scrollBy its Shift + wheel uses, which keeps the
  // mandatory snap stepping a card at a time.
  useEffect(() => {
    const el = strip.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      const c = carousel.current;
      if (e.shiftKey || Math.abs(e.deltaY) <= Math.abs(e.deltaX) || !c) return;
      if (!(e.deltaY > 0 ? c.canScrollNext() : c.canScrollPrev())) return;
      e.preventDefault();
      scrollArea(el)?.scrollBy({ left: e.deltaY, behavior: "auto" });
    };
    // A card's control is the SelectableCard's input beside its surface, so a press on a tile (its card, caption or
    // name) would hand focus to the nearest focusable box, the Carousel's scroll area, and the next key (paging the
    // results) would show its ring round the whole strip. A press leaves focus where it is; the keyboard still
    // reaches every card.
    const onPress = (e: MouseEvent) => {
      if (e.target instanceof Element && e.target.closest(".strip-tile"))
        e.preventDefault();
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    el.addEventListener("mousedown", onPress);
    return () => {
      el.removeEventListener("wheel", onWheel);
      el.removeEventListener("mousedown", onPress);
    };
  }, []);

  const row = useRef<Row>({
    chipX: [],
    length: 0,
    pitch: 0,
    ax: 0,
    state: folded ? "rest" : "none",
    chips: folded,
  });
  const use = useCallback(() => {
    if (row.current.state === "rest") lay(strip.current, row.current, true);
  }, []);
  useImperativeHandle(
    morph,
    () => ({
      settle(state) {
        const r = row.current;
        r.state = state;
        if (state !== "rest") lay(strip.current, r, false);
      },
      use,
    }),
    [use],
  );
  // the cards rendered and their names set the chips' places; a resize sets the row's end, a font the names' widths
  // biome-ignore lint/correctness/useExhaustiveDependencies: what is rendered changes the tiles measure reads
  useLayoutEffect(
    () => measure(strip.current, row.current),
    [all, avatars, ui],
  );
  useEffect(() => {
    const el = strip.current;
    const area = el && scrollArea(el);
    if (!el || !area) return;
    const place = () => anchor(el, row.current);
    const again = () => measure(el, row.current);
    // without scrollend (Safari), once the cards stop moving
    let timer = 0;
    const later = () => {
      clearTimeout(timer);
      timer = window.setTimeout(place, 150);
    };
    const end = "onscrollend" in window ? "scrollend" : "scroll";
    const moved = end === "scroll" ? later : place;
    const sized = new ResizeObserver(place);
    sized.observe(area);
    area.addEventListener(end, moved);
    document.fonts.addEventListener("loadingdone", again);
    for (const e of USE)
      el.addEventListener(e, use, { capture: true, passive: true });
    return () => {
      clearTimeout(timer);
      sized.disconnect();
      area.removeEventListener(end, moved);
      document.fonts.removeEventListener("loadingdone", again);
      for (const e of USE) el.removeEventListener(e, use, { capture: true });
    };
  }, [use]);

  return (
    <Carousel
      gap={2}
      hasSnap
      hasButtons={!touch}
      handleRef={carousel}
      ref={strip}
      aria-label={t("strip.label")}
      className="strip-cards"
      style={SIZES}
      data-chips={folded ? "" : undefined}
    >
      <Tile
        label={t("strip.all")}
        selected={!selected.size}
        onSelect={() => pick("")}
      />
      {(all ? avatars : avatars.slice(0, FIRST_CARDS - 1)).map((a) => (
        <Tile
          key={a.slug}
          label={avatarName(a, ui)}
          selected={selected.has(a.slug)}
          onSelect={() => pick(selected.has(a.slug) ? "" : a.slug)}
          src={a.image_url && imageSrc(a.image_url, "300x300_a2_g5")}
        />
      ))}
    </Carousel>
  );
});

// A card over its caption, the round face of its chip over the card; its chip's name beside where the card shrinks
// to, hidden from assistive technology as the caption already names the card, and drawn first, so the selected
// chip's fill behind it lies under the avatar too.
function Tile({
  label,
  selected,
  onSelect,
  src,
}: {
  label: string;
  selected: boolean;
  onSelect: () => void;
  /** the avatar's image; without one a glyph sits on the muted card surface, so the card still reads as one */
  src?: string | null;
}) {
  const image = (radius?: string) =>
    src ? (
      <SkeletonImage
        radius={radius}
        src={src}
        loading="lazy"
        decoding="async"
      />
    ) : (
      <AllGlyph />
    );
  return (
    <VStack
      gap={2}
      className="strip-tile"
      data-selected={selected ? "" : undefined}
    >
      <HStack vAlign="center" className="strip-name" aria-hidden>
        <Text
          type="supporting"
          weight={selected ? "semibold" : undefined}
          maxLines={1}
          hasTruncateTooltip={false}
        >
          {label}
        </Text>
      </HStack>
      <AspectRatio ratio={1} className="strip-thumb">
        <SelectableCard
          label={label}
          isSelected={selected}
          onChange={onSelect}
          variant={src ? "transparent" : "muted"}
          padding={THUMB_PADDING}
          width="100%"
          height="100%"
          className="strip-card"
        >
          {image(THUMB_RADIUS)}
        </SelectableCard>
        <AspectRatio
          ratio={1}
          shape="ellipse"
          fit="cover"
          className="strip-face"
          data-glyph={src ? undefined : ""}
          aria-hidden
        >
          {image()}
        </AspectRatio>
      </AspectRatio>
      <Text
        type="supporting"
        maxLines={1}
        justify="center"
        hasTruncateTooltip={false}
        className="strip-caption"
      >
        {label}
      </Text>
    </VStack>
  );
}
