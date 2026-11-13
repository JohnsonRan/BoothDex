// The filter rail's width (catalog/filter/filter-rail.tsx), dragged by the visitor and kept in a cookie of its own so
// the server draws the rail at that width from the first paint. Not a setting: the width suits one screen, so it
// stays in this browser and is not sent to the account (lib/preferences.ts). In rem, so the rail grows with the text.

export const RAIL_COOKIE = "rail";
export const RAIL_REM = { min: 14, default: 16, max: 24 } as const;

/** A stored width in rem, held to the range and to quarter rems; anything else is the default. */
export function parseRailWidth(raw: string | null | undefined): number {
  const rem = Number(raw);
  if (!raw || !Number.isFinite(rem)) return RAIL_REM.default;
  return Math.min(
    RAIL_REM.max,
    Math.max(RAIL_REM.min, Math.round(rem * 4) / 4),
  );
}

/** The cookie that keeps a width for a year. */
export const railCookie = (rem: number) =>
  `${RAIL_COOKIE}=${parseRailWidth(String(rem))}; path=/; max-age=31536000; samesite=lax`;

/** The width to write after a drag settles at `size` rem, or null when the cookie already holds it (`saved`, the
 *  value last written or read). Compared after parsing, so a width that only differs by rounding writes nothing. */
export function railToSave(saved: number, size: number): number | null {
  const next = parseRailWidth(String(size));
  return next === saved ? null : next;
}
