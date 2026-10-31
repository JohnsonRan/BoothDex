// Items opened, newest first. Signed out they stay in this browser; at sign-in they move into the account
// (/api/account/views), which then keeps them.

const KEY = "viewedItems";
/** same cap as the API keeps per account */
export const VIEWED_MAX = 500;

/** Newest first; opening an item again moves it to the top. */
export function addViewed(
  list: number[],
  id: number,
  max = VIEWED_MAX,
): number[] {
  return [id, ...list.filter((x) => x !== id)].slice(0, max);
}

export function loadViewed(): number[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v)
      ? v.filter((x) => Number.isSafeInteger(x) && x > 0).slice(0, VIEWED_MAX)
      : [];
  } catch {
    return [];
  }
}

export function saveViewed(list: number[]) {
  try {
    if (list.length) localStorage.setItem(KEY, JSON.stringify(list));
    else localStorage.removeItem(KEY);
  } catch {
    // storage full or blocked (private mode): history just isn't kept
  }
}
