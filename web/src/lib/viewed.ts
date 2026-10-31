// Items opened, newest first with their view timestamp. Signed out they stay in this browser; at sign-in
// they move into the account (/api/account/views), which then keeps them.

const KEY = "viewedItems";
/** same cap as the API keeps per account */
export const VIEWED_MAX = 500;

export type ViewedPair = [id: number, at: number];

/** Newest first; opening an item again moves it to the top. */
export function addViewed(
  list: number[],
  id: number,
  max = VIEWED_MAX,
): number[] {
  return [id, ...list.filter((x) => x !== id)].slice(0, max);
}

/** Parses [id, at] pairs, dropping invalid or old number-only entries and keeping newest first. */
export function parseViewed(raw: unknown, max = VIEWED_MAX): ViewedPair[] {
  if (!Array.isArray(raw)) return [];
  const valid: ViewedPair[] = [];
  const seen = new Set<number>();
  for (const item of raw) {
    if (
      Array.isArray(item) &&
      item.length >= 2 &&
      Number.isSafeInteger(item[0]) &&
      item[0] > 0 &&
      typeof item[1] === "number" &&
      Number.isFinite(item[1])
    ) {
      if (!seen.has(item[0])) {
        seen.add(item[0]);
        valid.push([item[0], item[1]]);
        if (valid.length >= max) break;
      }
    }
  }
  return valid;
}

export function loadViewed(): ViewedPair[] {
  try {
    return parseViewed(JSON.parse(localStorage.getItem(KEY) ?? "[]"));
  } catch {
    return [];
  }
}

export function saveViewed(list: ViewedPair[]) {
  try {
    if (list.length) localStorage.setItem(KEY, JSON.stringify(list));
    else localStorage.removeItem(KEY);
  } catch {
    // storage full or blocked (private mode): history just isn't kept
  }
}

/** Prepends [id, at] to localStorage, moving any earlier record of id to the top. */
export function recordLocalView(
  id: number,
  at = Date.now(),
  max = VIEWED_MAX,
): ViewedPair[] {
  const entry: ViewedPair = [id, at];
  const current = loadViewed().filter(([x]) => x !== id);
  const next: ViewedPair[] = [entry, ...current].slice(0, max);
  saveViewed(next);
  return next;
}
