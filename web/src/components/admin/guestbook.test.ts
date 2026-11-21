import { expect, test } from "bun:test";
import { listView, pageOf } from "./guestbook";
import type { AdminGuestbookEntry } from "./types";

const entry = (id: number) => ({ id }) as AdminGuestbookEntry;

test("every loaded row deleted while older messages remain reads the next page, from the last row loaded", () => {
  const page = pageOf({ entries: [entry(60), entry(11)], more: true });
  expect(page.cursor).toBe(11);
  // the admin deletes both rows: the list reads on from 11 instead of showing the empty state
  const deleted = { ...page, entries: [] };
  expect(listView(deleted)).toBe("next");
  expect(deleted.cursor).toBe(11);
  // the next page's last row moves the cursor; an empty answer keeps it
  expect(
    pageOf({ entries: [entry(10), entry(1)], more: false }, 11).cursor,
  ).toBe(1);
  expect(pageOf({ entries: [], more: false }, 11).cursor).toBe(11);
  expect(listView({ entries: [], more: false })).toBe("empty");
  expect(listView(page)).toBe("rows");
});
