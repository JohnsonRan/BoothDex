// Shared values of the filter row (table-filter template, page.tsx:1129-1160 and its StyleX rules :1303-1559),
// written as token-backed inline styles because the site has no StyleX compiler. The rules that need a pseudo-class
// or a container query are plain classes in app/filter-row.css.

import { type CSSProperties, useEffect, useState } from "react";
import type { Recognized } from "@/lib/api-types";

/** What the clauses show once the search answers: the count, and per category and feature counts. */
export type Facets = {
  count: number;
  smart: Record<string, number>;
  features: Record<string, number>;
  /** what the search read the query's words as: their tokens show it (search-suggest.tsx) */
  marks: SearchMark[];
  /** the query the search answered, `q` as the URL holds it: the marks belong to it alone */
  query: string;
};

/** Query words the search took as a model or a smart category (the API's `recognized`), named for their token. */
export type SearchMark = Recognized & {
  /** in the visitor's language */
  name: string;
  /** a model's picture */
  image?: string;
};

/** Every control of the toolbar row is the template's `sm`. */
export const CONTROL_SIZE = "sm";

/** Below `md`, or a touch tablet: folded clauses and the forms open as sheets (COMPACT_SURFACE_QUERY, :1129). */
export const COMPACT_SURFACE_QUERY =
  "(max-width: 768px), (max-width: 1024px) and (pointer: coarse) and (hover: none)";
/** Too narrow for the filters' rail (catalog/filter-rail.tsx): a side panel earns its width above 1024 px
 *  (`astryx docs layout/responsive`), below which the filters stay in the row. */
export const NO_RAIL_QUERY = "(max-width: 1024px)";
/** Sheet content starts below the grab pill's band (SHEET_TOP_INSET, :1142). */
export const SHEET_TOP_INSET = 6;
/** The panels' widths in rem, so they grow with the text they hold, as the controls beside them do (TEMPLATE-MAP
 *  rule 4): the price and tag editors at quotePopover's 300 px (:1545-1547), the folded clauses at
 *  FilterOverflow's 280 px (:2099). */
export const EDITOR_WIDTH = "18.75rem";
export const OVERFLOW_WIDTH = "17.5rem";

// filterChrome + filterSurface (:1526-1534): ToggleButton is a ghost button, so unpressed it has no chrome; the
// border puts it in the same family as the selectors beside it.
export const FILTER_CHROME: CSSProperties = {
  borderWidth: "var(--border-width)",
  borderStyle: "solid",
  borderColor: "var(--color-border-emphasized)",
  borderRadius: "var(--radius-element)",
};
export const FILTER_SURFACE: CSSProperties = {
  backgroundColor: "var(--color-background-surface)",
};
// filterFill (:1539): a control carrying a value takes the token ToggleButton paints when pressed.
export const FILTER_FILL: CSSProperties = {
  backgroundColor: "var(--color-overlay-pressed)",
};

// filterLabelValue / filterLabelEmpty (:1553-1559): a Selector states its emptiness typographically; button labels
// are medium and primary, so the toggles take the selectors' type.
const FILTER_LABEL_VALUE: CSSProperties = {
  fontWeight: "var(--font-weight-normal)",
};
const FILTER_LABEL_EMPTY: CSSProperties = {
  fontWeight: "var(--font-weight-normal)",
  color: "var(--color-text-secondary)",
};

/** A preset or chip ToggleButton's style, pressed or not (template :3292-3296). */
export const toggleStyle = (isPressed: boolean): CSSProperties =>
  isPressed
    ? { ...FILTER_CHROME, ...FILTER_LABEL_VALUE }
    : { ...FILTER_CHROME, ...FILTER_LABEL_EMPTY, ...FILTER_SURFACE };

/** The facets once the search answers, null before: the row renders at once and never waits for them. */
export function useFacets(facets: Promise<Facets>): Facets | null {
  const [value, setValue] = useState<Facets | null>(null);
  useEffect(() => {
    let live = true;
    facets.then((v) => live && setValue(v));
    return () => {
      live = false;
    };
  }, [facets]);
  return value;
}
