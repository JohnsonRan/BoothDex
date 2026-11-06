// Shared values of the filter row (table-filter template, page.tsx:1129-1160 and its StyleX rules :1303-1559),
// written as token-backed inline styles because the site has no StyleX compiler. The rules that need a pseudo-class
// or a container query are plain classes in app/filter-row.css.

import { type CSSProperties, useEffect, useState } from "react";

/** What the clauses show once the search answers: the count, and per category and feature counts. */
export type Facets = {
  count: number;
  smart: Record<string, number>;
  features: Record<string, number>;
};

/** Every control of the toolbar row is the template's `sm`. */
export const CONTROL_SIZE = "sm";

/** Below `md`, or a touch tablet: folded clauses and the forms open as sheets (COMPACT_SURFACE_QUERY, :1129). */
export const COMPACT_SURFACE_QUERY =
  "(max-width: 768px), (max-width: 1024px) and (pointer: coarse) and (hover: none)";
/** Sheet content starts below the grab pill's band (SHEET_TOP_INSET, :1142). */
export const SHEET_TOP_INSET = 6;
/**
 * Width the clause slot holds back for the readout after the clauses (READOUT_RESERVE, :1160): paid as padding,
 * which the overflow measurement subtracts, and taken back by the readout's negative margin. In rem, so it grows
 * with the text it reserves room for.
 */
export const READOUT_RESERVE = "11.5rem";

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
