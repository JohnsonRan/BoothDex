// Pagination state helpers for the admin guestbook list (guestbook.tsx).

import type { AdminGuestbookEntry } from "./types";

/** The rows loaded so far, whether older ones remain, and the id the next page is read from: the last row loaded,
 *  kept when rows are deleted. */
export type Page = {
  entries: AdminGuestbookEntry[];
  more: boolean;
  cursor?: number;
};

/** An answer of the API as a page, read on from its last row, or from `cursor` when it has none. */
export const pageOf = (
  r: { entries: AdminGuestbookEntry[]; more: boolean },
  cursor?: number,
): Page => ({ ...r, cursor: r.entries.at(-1)?.id ?? cursor });

/** What the list shows: its rows; the next page, when every loaded row was deleted and older messages remain; or the
 *  empty state. */
export function listView(page: Page): "rows" | "next" | "empty" {
  if (page.entries.length > 0) return "rows";
  return page.more ? "next" : "empty";
}
