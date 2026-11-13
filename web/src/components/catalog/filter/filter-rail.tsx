"use client";
// The filters in a rail left of the results, for a visitor who puts them at the side (Settings, settings-core
// FILTER_PLACES): the page Layout's start panel (Layout `start` with LayoutPanel, Layout.doc "Page with a
// sidebar"), set apart from the results by space, not a divider: the divider met the band's lower edge and the
// shrunk avatar strip's in a grid of lines. The fields are a compact CollapsibleGroup with dividers, every
// section open (block CollapsibleGroupAccordion: compact for sidebars, triggers in body semibold, not the
// trigger's own `large`), its
// choices laid out as a shop's sidebar does: one of many as a RadioList led by its "any" choice, the features as
// a CheckboxList, both with the result counts the row's menus show. The models are too many to lay out and the tags
// have no fixed list, so those keep their pickers. The search, the count and the sort stay in the toolbar
// (filter-bar.tsx). It sticks under the top bar as the band does (band.tsx), scrolling on its own when taller than
// the window. At 1024 px and below it has no room (NO_RAIL_QUERY): the row keeps the clauses there
// (app/filter-row.css hides the server's rail before this knows).

import {
  CheckboxList,
  CheckboxListItem,
} from "@astryxdesign/core/CheckboxList";
import { Collapsible, CollapsibleGroup } from "@astryxdesign/core/Collapsible";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { LayoutPanel } from "@astryxdesign/core/Layout";
import { RadioList, RadioListItem } from "@astryxdesign/core/RadioList";
import { ResizeHandle, useResizable } from "@astryxdesign/core/Resizable";
import { Selector } from "@astryxdesign/core/Selector";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Switch } from "@astryxdesign/core/Switch";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import {
  type CSSProperties,
  memo,
  type ReactNode,
  type RefObject,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { useUi } from "@/components/providers";
import { FEATURES, type Feature, type Filters, RANGES } from "@/lib/catalog";
import { RAIL_REM, railCookie, railToSave } from "@/lib/rail-width";
import { LOCALES } from "@/lib/settings-core";
import {
  CLAUSE_CLEAR,
  type ClauseNames,
  clauseTexts,
  digits,
  PRICE_PRESETS,
  presetKey,
  priceText,
} from "./clause-text";
import { type Facets, NO_RAIL_QUERY, useFacets } from "./filter-style";
import { useFilterUrl } from "./filter-url";
import { TagClause } from "./tag-clause";

// the top bar's height, as band.tsx reads it
const TOP = "var(--_app-shell-header-height, 0px)";
const RAIL: CSSProperties = {
  position: "sticky",
  insetBlockStart: TOP,
  // its own height, not the row's, so it can stick; the window's, so its divider runs the window's height
  alignSelf: "flex-start",
  blockSize: `calc(100dvh - ${TOP})`,
};

// `count`: the search's count for the choice, null until it answers; a choice it does not count leaves it out
type Choice = { value: string; label: string; count?: number | null };

// every section, open from the start
const SECTIONS = [
  "primary",
  "avatar",
  "rating",
  "price",
  "time",
  "features",
  "tags",
];

export function FilterRail({
  facets,
  names,
  width,
}: {
  facets: Promise<Facets>;
  names: ClauseNames;
  /** in rem, as the visitor last dragged it (lib/rail-width.ts) */
  width: number;
}) {
  const { t } = useUi();
  const [rem, setRem] = useState(width);
  // the width the cookie holds: the one the page loaded with, then each one written
  const saved = useRef(width);
  const isNarrow = useMediaQuery(NO_RAIL_QUERY);
  if (isNarrow) return null;
  return (
    <>
      <LayoutPanel
        role="region"
        label={t("filter.rail")}
        width={`${rem}rem`}
        padding={0}
        className="filter-rail"
        style={RAIL}
      >
        <RailFields facets={facets} names={names} />
      </LayoutPanel>
      <RailHandle rem={rem} saved={saved} onResize={setRem} />
    </>
  );
}

// the root font size changes with the window (app/globals.css), so a resize may change how many pixels a rem is
// read once and again on each resize, not on every render of a drag
const readRem = () =>
  Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
let remPx = 0;
const remPixels = () => {
  if (!remPx) remPx = readRem();
  return remPx;
};
const onWindowResize = (change: () => void) => {
  const resized = () => {
    remPx = readRem();
    change();
  };
  addEventListener("resize", resized);
  return () => removeEventListener("resize", resized);
};

/**
 * The rail's resize handle, beside it at the content's edge. It mounts once the page has hydrated: the hook counts in
 * pixels, and only the browser knows how many a rem is, while the server has already drawn the rail at the stored
 * width in rem. Without a divider the handle takes no width, so mounting it moves nothing. A double-click goes back to
 * the default width, as a split view's divider does; the arrow keys step the width (ResizeHandle).
 */
function RailHandle({
  rem,
  saved,
  onResize,
}: {
  rem: number;
  saved: RefObject<number>;
  onResize: (rem: number) => void;
}) {
  // 0 on the server and while hydrating; a new size of a rem starts the hook again from the width in rem
  const px = useSyncExternalStore(onWindowResize, remPixels, () => 0);
  return px ? (
    <RailHandleControl
      key={px}
      px={px}
      rem={rem}
      saved={saved}
      onResize={onResize}
    />
  ) : null;
}

function RailHandleControl({
  px,
  rem,
  saved,
  onResize,
}: {
  px: number;
  rem: number;
  saved: RefObject<number>;
  onResize: (rem: number) => void;
}) {
  const { t, ui } = useUi();
  const region = useResizable({
    defaultSize: rem * px,
    minSize: RAIL_REM.min * px,
    maxSize: RAIL_REM.max * px,
  });
  const size = region.size / px;
  useEffect(() => onResize(size), [size, onResize]);
  // written at once, so a reload right after a change keeps it: the cookie holds quarter rems, so a drag writes it
  // only when it crosses one (about every 4 px), not on every frame. Not on unload: the browser sends the reload's
  // request before the old page's pagehide runs.
  useEffect(() => {
    const next = railToSave(saved.current, size);
    if (next === null) return;
    // biome-ignore lint/suspicious/noDocumentCookie: the server reads this cookie; Cookie Store is not in every browser
    document.cookie = railCookie(next);
    saved.current = next;
  }, [size, saved]);
  return (
    <ResizeHandle
      resizable={region}
      label={t("filter.railResize")}
      // the handle reports pixels; a screen reader reads the width in rem, as it is set and stored
      aria-valuetext={t("filter.railWidth", {
        width: size.toLocaleString(LOCALES[ui], { maximumFractionDigits: 1 }),
      })}
      isAlwaysVisible={false}
      className="filter-rail-handle"
      onDoubleClick={() => region.resize(RAIL_REM.default * px)}
    />
  );
}

/** The rail's fields, kept from rendering again on every frame of a drag, which changes only the rail's width. */
const RailFields = memo(function RailFields({
  facets,
  names,
}: {
  facets: Promise<Facets>;
  names: ClauseNames;
}) {
  const { t } = useUi();
  const { filters: f, apply } = useFilterUrl();
  const answered = useFacets(facets);
  const avatars = f.view === "avatars";
  const any = t("filter.timeAny");

  // one of many, its first choice the field unset
  const radios = (
    section: string,
    field: string,
    value: string,
    choices: Choice[],
    onChange: (v: string) => void,
  ) => (
    <Section value={section} field={field}>
      <RadioList
        label={field}
        isLabelHidden
        size="sm"
        value={value}
        onChange={onChange}
      >
        {choices.map((c) => (
          <RadioListItem
            key={c.value}
            value={c.value}
            label={c.label}
            endContent={c.count !== undefined && <Count n={c.count} />}
          />
        ))}
      </RadioList>
    </Section>
  );

  return (
    <>
      {/* the band's toolbar inset above (band.tsx), so the first section lines up with the search */}
      <VStack
        gap={4}
        paddingInlineStart={6}
        paddingInlineEnd={4}
        paddingBlockStart={4}
        paddingBlockEnd={6}
      >
        {/* the shop has no field of its own: a link sets it, and this is how it is removed */}
        {f.author && (
          <Selector
            label={f.author}
            isLabelHidden
            size="sm"
            hasClear
            options={[f.author]}
            value={f.author}
            onChange={(v) => {
              if (v == null) apply(CLAUSE_CLEAR.author);
            }}
          />
        )}
        <CollapsibleGroup
          type="multiple"
          hasDividers
          density="compact"
          defaultValue={SECTIONS}
        >
          {avatars
            ? radios(
                "primary",
                t("filter.gender"),
                f.gender,
                [
                  { value: "", label: t("filter.all") },
                  { value: "female", label: t("filter.female") },
                  { value: "male", label: t("filter.male") },
                ],
                (v) => apply({ gender: v as Filters["gender"] }),
              )
            : radios(
                "primary",
                t("filter.categories"),
                f.sc,
                [
                  // no count: the search's is for the category chosen, not for all
                  { value: "", label: t("filter.all") },
                  ...names.categories.map(([value, label]) => ({
                    value,
                    label,
                    count: answered ? (answered.smart[value] ?? 0) : null,
                  })),
                ],
                (sc) => apply({ sc }),
              )}
          {/* an avatar body is not fitted to another avatar */}
          {!avatars && (
            <Section value="avatar" field={t("filter.avatar")}>
              <Selector
                label={t("filter.avatar")}
                isLabelHidden
                placeholder={t("filter.all")}
                size="sm"
                hasClear
                hasSearch
                options={names.avatars.map(([value, label]) => ({
                  value,
                  label,
                }))}
                value={f.avatar || null}
                onChange={(v) =>
                  apply(v == null ? CLAUSE_CLEAR.avatar : { avatar: v })
                }
              />
            </Section>
          )}
          {radios(
            "rating",
            t("filter.rating"),
            f.adult === "include" ? "include" : "safe",
            [
              { value: "safe", label: t("filter.safe") },
              { value: "include", label: t("filter.all") },
            ],
            (v) => apply({ adult: v === "include" ? "include" : "safe" }),
          )}
          <PriceSection />
          {radios(
            "time",
            t("filter.time"),
            f.range,
            [
              { value: "", label: any },
              ...RANGES.map((r) => ({ value: r, label: t(`range.${r}`) })),
            ],
            (v) => apply({ range: v as Filters["range"] }),
          )}
          <Section value="features" field={t("filter.features")}>
            <CheckboxList
              label={t("filter.features")}
              isLabelHidden
              density="compact"
              value={f.features}
              onChange={(v) => apply({ features: v as Feature[] })}
            >
              {(Object.keys(FEATURES) as Feature[]).map((x) => (
                <CheckboxListItem
                  key={x}
                  value={x}
                  label={FEATURES[x]}
                  endContent={
                    <Count n={answered ? (answered.features[x] ?? 0) : null} />
                  }
                />
              ))}
            </CheckboxList>
          </Section>
          <Section value="tags" field={t("filter.tags")}>
            <TagClause
              label={clauseTexts(f, names, t).tags}
              placeholder={t("filter.tagSearch")}
            />
          </Section>
        </CollapsibleGroup>
        <VStack gap={3}>
          {f.view !== "favourites" && (
            <Switch
              label={t("view.avatars")}
              size="sm"
              value={avatars}
              onChange={(on) =>
                apply({
                  view: on ? "avatars" : "items",
                  avatar: "",
                  sc: "",
                  gender: "",
                })
              }
            />
          )}
          {avatars && (
            <Switch
              label={t("filter.supportedOnly")}
              size="sm"
              value={f.asup === "supported"}
              onChange={(on) => apply({ asup: on ? "supported" : "all" })}
            />
          )}
        </VStack>
      </VStack>
    </>
  );
});

/** A field's section in the group: its name as the trigger, open until folded by hand. */
function Section({
  value,
  field,
  children,
}: {
  value: string;
  field: string;
  children: ReactNode;
}) {
  return (
    <Collapsible
      value={value}
      trigger={
        <Text type="body" weight="semibold">
          {field}
        </Text>
      }
    >
      {children}
    </Collapsible>
  );
}

// the widest count a choice shows: six digits, which the catalogue (about 216,000 items, 2026-11) does not outgrow
const WIDEST = 999_999;
const SIZER: CSSProperties = {
  visibility: "hidden",
  blockSize: 0,
  overflow: "hidden",
};

/**
 * A choice's result count, once the search answers (`n` null until then). From the first paint its slot is as
 * wide as the widest count, held by an unseen flat copy, so the label beside it keeps its line breaks when the
 * count arrives or a refresh clears it: in English, a category's name grew a line and moved every section below.
 * The number goes without the count's unit ("件", "items"), as a shop's sidebar shows it: six bare digits take less
 * room than five with the unit did, so no name beside a count takes another line.
 */
function Count({ n }: { n: number | null }) {
  const { ui } = useUi();
  const text = (v: number) => v.toLocaleString(LOCALES[ui]);
  return (
    <VStack hAlign="end">
      <Text type="supporting" hasTabularNumbers style={SIZER}>
        {text(WIDEST)}
      </Text>
      {n !== null && (
        <Text type="supporting" color="secondary" hasTabularNumbers>
          {text(n)}
        </Text>
      )}
    </VStack>
  );
}

/** The price: any, free or a preset band as radios, then a range of its own typed in, applied on Enter. */
function PriceSection() {
  const { t } = useUi();
  const { filters: f, apply } = useFilterUrl();
  const preset = PRICE_PRESETS.find(
    ([lo, hi]) => !f.free && f.pmin === lo && f.pmax === hi,
  );
  const value = f.free
    ? "free"
    : preset
      ? presetKey(preset)
      : f.pmin || f.pmax
        ? ""
        : "any";
  return (
    <Section value="price" field={t("filter.price")}>
      <VStack gap={3}>
        <RadioList
          label={t("filter.price")}
          isLabelHidden
          size="sm"
          value={value}
          onChange={(v) => {
            const p = PRICE_PRESETS.find((x) => presetKey(x) === v);
            apply({ free: v === "free", pmin: p?.[0] ?? 0, pmax: p?.[1] ?? 0 });
          }}
        >
          <RadioListItem value="any" label={t("filter.timeAny")} />
          <RadioListItem value="free" label={t("price.free")} />
          {PRICE_PRESETS.map((p) => (
            <RadioListItem
              key={presetKey(p)}
              value={presetKey(p)}
              label={priceText({ pmin: p[0], pmax: p[1] })}
            />
          ))}
        </RadioList>
        {/* a new range from the URL starts the inputs over */}
        <PriceRange key={`${f.pmin}-${f.pmax}`} pmin={f.pmin} pmax={f.pmax} />
      </VStack>
    </Section>
  );
}

function PriceRange({ pmin, pmax }: { pmin: number; pmax: number }) {
  const { t } = useUi();
  const { apply } = useFilterUrl();
  const [lo, setLo] = useState(pmin);
  const [hi, setHi] = useState(pmax);
  // reversed bounds are swapped, as the price editor does
  const commit = () =>
    apply({
      free: false,
      pmin: hi && lo > hi ? hi : lo,
      pmax: hi && lo > hi ? lo : hi,
    });
  const input = (label: string, v: number, set: (v: number) => void) => (
    <TextInput
      label={label}
      isLabelHidden
      size="sm"
      placeholder={`¥ ${label}`}
      inputMode="numeric"
      value={v ? String(v) : ""}
      onChange={(s) => set(digits(s))}
      onEnter={commit}
    />
  );
  return (
    <HStack gap={2} vAlign="center">
      {input(t("filter.min"), lo, setLo)}
      <Text type="supporting" color="secondary">
        –
      </Text>
      {input(t("filter.max"), hi, setHi)}
    </HStack>
  );
}
