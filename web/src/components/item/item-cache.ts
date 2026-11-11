// The item dialog's records, kept apart from the card itself (item-dialog.tsx drops the rendered card once it
// closes): an item opened again, or brought back by Back and Forward, shows at once. A record is fetched when the
// visitor opens the item, already as they press its tile (item-host.tsx), and kept for a few minutes;
// the last 50 are kept. Only the record (`/api/items`, a plain read): the gallery request counts a view and may
// ask Booth, so it is sent by the dialog, once per open, and only its answer is kept.
// Before any record, the card draws what the item's tile showed (keepTiles): the shop, the cover, the title and
// the price are on screen with the click.

import type { Tile } from "@/components/catalog/item-tile";
import { api } from "@/lib/api";
import type { ItemResponse } from "@/lib/api-types";

const TILES = 500;
const tiles = new Map<number, Tile>();

/** Remembers what these tiles show (catalog/tiles.tsx), the last 500. */
export function keepTiles(list: Tile[]): void {
  for (const tile of list) {
    tiles.delete(tile.id);
    tiles.set(tile.id, tile);
  }
  for (const id of tiles.keys()) {
    if (tiles.size <= TILES) break;
    tiles.delete(id);
  }
}

/** What item `id`'s tile showed, if the page drew one. */
export const keptTile = (id: number): Tile | undefined => tiles.get(id);

const KEEP = 50;
const FRESH_MS = 5 * 60_000;

type Entry = {
  at: number;
  load: Promise<ItemResponse | null>;
  /** the record once it arrived, with the whole gallery once that did */
  item?: ItemResponse;
  gallery?: string[];
};
const entries = new Map<number, Entry>();

const withGallery = (item: ItemResponse, gallery?: string[]): ItemResponse =>
  gallery ? { ...item, booth_images: gallery, media: [] } : item;

/** Item `id`'s record, from memory when it is fresh; null when the API knows no such item. */
export function loadItem(id: number): Promise<ItemResponse | null> {
  const kept = entries.get(id);
  if (kept && Date.now() - kept.at < FRESH_MS) return kept.load;
  const drop = () => entries.get(id) === entry && entries.delete(id);
  const entry: Entry = {
    at: Date.now(),
    // an old record shows until the new one arrives
    item: kept?.item,
    gallery: kept?.gallery,
    load: api<{ items: ItemResponse[] }>(`/api/items?ids=${id}`).then(
      (r) => {
        const item = r.items[0];
        if (!item) {
          drop();
          return null;
        }
        entry.item = withGallery(item, entry.gallery);
        return entry.item;
      },
      (e) => {
        // a failed fetch is tried again next time
        drop();
        throw e;
      },
    ),
  };
  entries.delete(id);
  entries.set(id, entry);
  // the oldest goes (Map keeps insertion order)
  if (entries.size > KEEP) entries.delete(entries.keys().next().value ?? id);
  return entry.load;
}

/** Fetches item `id`'s record ahead of an open; a failure is left for the open to report. */
export const prefetchItem = (id: number) => void loadItem(id).catch(() => {});

/** Item `id`'s record if it has arrived. */
export const keptItem = (id: number): ItemResponse | undefined =>
  entries.get(id)?.item;

/** Keeps Booth's whole gallery for item `id`, in place of the record's images. */
export function keepGallery(id: number, gallery: string[]): void {
  const entry = entries.get(id);
  if (!entry) return;
  entry.gallery = gallery;
  if (entry.item) entry.item = withGallery(entry.item, gallery);
}
