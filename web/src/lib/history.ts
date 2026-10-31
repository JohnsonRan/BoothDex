// Recent searches, kept in this browser only (like the remembered avatar for signed-out visitors).

const KEY = "searchHistory";
export const HISTORY_MAX = 10;

/** Newest first; the same query typed again moves to the top instead of repeating. */
export function addQuery(
  list: string[],
  q: string,
  max = HISTORY_MAX,
): string[] {
  const t = q.trim();
  if (!t) return list;
  const key = t.toLowerCase();
  return [t, ...list.filter((x) => x.toLowerCase() !== key)].slice(0, max);
}

export function loadHistory(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v)
      ? v.filter((x) => typeof x === "string").slice(0, HISTORY_MAX)
      : [];
  } catch {
    return [];
  }
}

export function saveHistory(list: string[]) {
  try {
    if (list.length) localStorage.setItem(KEY, JSON.stringify(list));
    else localStorage.removeItem(KEY);
  } catch {
    // storage full or blocked (private mode): history just isn't kept
  }
}
