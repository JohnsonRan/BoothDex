// Responsive Grid columns whose floor is in rem, so it follows the root scale (globals.css) the way the owner's
// browser zoom did. Grid's own `columns={{minWidth}}` takes px only, so this is the track list Grid writes for
// it (`repeat(auto-fill, minmax(<min>px, 1fr))`, Grid.js), given as the `style` Grid merges after its class,
// with `min(100%, ...)` so a lone column never overflows a phone. With `pairGap` (the Grid's `gap` step) the floor
// is also at most half the row less one gap, so two columns always fit however narrow the container is.

/** Grid props for `columns={{minWidth, repeat}}` with `minWidth` a rem length. */
export const remColumns = (
  minWidth: string,
  repeat: "fill" | "fit" = "fill",
  pairGap?: number,
) =>
  ({
    style: {
      gridTemplateColumns: `repeat(auto-${repeat}, minmax(min(100%, ${minWidth}${pairGap === undefined ? "" : `, (100% - var(--spacing-${pairGap})) / 2`}), 1fr))`,
    },
  }) as const;

/**
 * The columns of a grid from its resolved track list (`getComputedStyle(grid).gridTemplateColumns`), read with
 * `grid-auto-columns: 0px`: the implicit tracks an item spanning past the last column adds then resolve to 0px and
 * are not counted. Each auto-filled column is wider than 0 in a laid-out grid.
 */
export const columnCount = (tracks: string) =>
  tracks.split(" ").filter(Number.parseFloat).length;
