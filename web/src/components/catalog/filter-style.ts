// Shared values of the filter row (table-filter template): the control size and the template's StyleX rules,
// written as token-backed inline styles because the site has no StyleX compiler.

import { type CSSProperties, useEffect, useState } from "react";

/** What the clauses show once the search answers: per feature counts (and, for the toolbar, categories). */
export type Facets = {
  count: number;
  smart: Record<string, number>;
  features: Record<string, number>;
};

/** The control size of the whole control block (search, tabs, sort, clauses): the library template's `lg`. */
export const CONTROL_SIZE = "lg";

// filterChrome + filterSurface: ToggleButton is a ghost button, so unpressed it has no chrome; the border
// puts it in the same family as the selectors beside it.
export const FILTER_CHROME: CSSProperties = {
  borderWidth: "var(--border-width)",
  borderStyle: "solid",
  borderColor: "var(--color-border-emphasized)",
  borderRadius: "var(--radius-element)",
};
export const FILTER_SURFACE: CSSProperties = {
  backgroundColor: "var(--color-background-surface)",
};
// filterFill: a control carrying a value takes the token ToggleButton paints when pressed.
export const FILTER_FILL: CSSProperties = {
  backgroundColor: "var(--color-overlay-pressed)",
};

// filterLabelValue / filterLabelEmpty: a Selector states its emptiness typographically (normal weight,
// secondary text unset); button labels are medium and primary, so the toggles take the selectors' type.
export const FILTER_LABEL_VALUE: CSSProperties = {
  fontWeight: "var(--font-weight-normal)",
};
export const FILTER_LABEL_EMPTY: CSSProperties = {
  fontWeight: "var(--font-weight-normal)",
  color: "var(--color-text-secondary)",
};

/** "Field: value", the set clause's trigger text. */
export const clause = (field: string, value: string) => `${field}: ${value}`;

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
