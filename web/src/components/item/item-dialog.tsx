"use client";
// The item dialog: page template product-detail (templates/pages/product-detail/page.tsx) inside an Astryx
// Dialog, as blocks DialogFullscreenDialog and DialogAdaptivePresentation compose one: a Layout with a
// DialogHeader, a LayoutContent and a LayoutFooter. A wide standard dialog from lg up, the fullscreen variant
// below it (master: full screen on phones), where the footer carries price, favourite and Buy on Booth.
// The header names the shop (block DialogWithSubtitle: title and subtitle, avatar as start content), with the
// link to the shop's other items in the subtitle beside its host, not by the close button; the dialog itself is labelled by the item's title. Inside, the template's two columns (gallery, info), then
// the "More from" strip across both.

import { Avatar } from "@astryxdesign/core/Avatar";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Grid } from "@astryxdesign/core/Grid";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { Link } from "@astryxdesign/core/Link";
import { VStack } from "@astryxdesign/core/Stack";
import { use, useEffect, useRef, useState } from "react";
import { useAccount } from "@/components/account/session";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { ItemResponse } from "@/lib/api-types";
import { imageSrc } from "@/lib/catalog";
import { DIALOG_MAX_HEIGHT } from "@/lib/dialog-height";
import { remColumns } from "@/lib/grid";
import type { Rates } from "@/lib/settings-core";
import { useDelayed } from "@/lib/use-delayed";
import { useScrollTop } from "@/lib/use-scroll-top";
import { PhoneBar } from "./item-actions";
import { keepGallery, keptItem, keptTile, loadItem } from "./item-cache";
import { shopHost } from "./item-data";
import { ItemGallery } from "./item-gallery";
import { useItemOpener } from "./item-host";
import { ItemInfo } from "./item-info";
import {
  DetailSilhouette,
  HeaderSilhouette,
  PhoneBarSilhouette,
} from "./item-silhouette";
import { MoreFrom } from "./more-from";
import "./item-icons";

/** Below lg the dialog is full screen (master's breakpoint). */
const PHONE = "(max-width: 1023px)";
// template: Layout contentWidth={1200}, in rem so it scales with the root (globals.css)
const WIDTH = "75rem";
// template: Grid columns={{minWidth: 320, repeat: 'fit'}} gap={5}, the floor in rem
const COLUMNS = remColumns("20rem", "fit");

type Gallery = { images: string[]; complete: boolean };

/** The full record (cards carry no bio, variations or avatars) and, once the API has it, Booth's whole gallery. */
function useItem(id: number): ItemResponse | null {
  const { notify, recordView } = useAccount();
  const { close } = useItemOpener();
  const [item, setItem] = useState<ItemResponse | null>(null);
  // an unknown or failed item: error toast, parameter removed (master browse.tsx:347-366)
  const failed = useRef(() => {});
  failed.current = () => {
    notify("list.loadFailed", "error");
    close();
  };
  // remembers the opened item (in the account when signed in, else in this browser), read when it arrives
  const viewed = useRef(recordView);
  viewed.current = recordView;
  useEffect(() => {
    if (!id) {
      // Closed: the item leaves once the closing frame is painted. Kept, the next item's open would first take
      // the whole card down, which doubled the time from a click to the dialog on screen.
      let timer = 0;
      const frame = requestAnimationFrame(() => {
        timer = window.setTimeout(() => setItem(null));
      });
      return () => {
        cancelAnimationFrame(frame);
        clearTimeout(timer);
      };
    }
    let live = true;
    const show = () => {
      const it = keptItem(id);
      if (live && it) setItem(it);
    };
    const fail = () => live && failed.current();
    loadItem(id).then((it) => {
      if (!it) return fail();
      show();
      if (live) viewed.current(id);
    }, fail);
    // once per open: it counts the view
    api<Gallery>(`/api/items/${id}/gallery`)
      .then((g) => {
        if (!g.complete || !g.images.length) return;
        keepGallery(id, g.images);
        show();
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [id]);
  // while closing (id 0) the last item stays until it is cleared above; a kept record shows at once
  if (!id) return item;
  return item?.id === id ? item : (keptItem(id) ?? null);
}

export default function ItemDialog({
  id,
  rates,
}: {
  id: number;
  rates: Promise<Rates>;
}) {
  const { t } = useUi();
  const { close } = useItemOpener();
  const known = use(rates);
  const phone = useMediaQuery(PHONE);
  const item = useItem(id);
  const shown = useDelayed(!!id && !item);
  // each item opens at its top, also one picked from "More from" down the card
  const body = useScrollTop(id);
  // until the record arrives, what the item's tile showed
  const tile = item || !id ? undefined : keptTile(id);
  const shop = item
    ? { name: item.author_name, icon: imageSrc(item.author_icon) }
    : tile && { name: tile.shop, icon: tile.shopIcon };

  return (
    <Dialog
      isOpen={id > 0}
      onOpenChange={(open) => !open && close()}
      variant={phone ? "fullscreen" : "standard"}
      width={WIDTH}
      maxHeight={DIALOG_MAX_HEIGHT}
      aria-label={item?.title ?? tile?.title ?? t("detail.label")}
    >
      <Layout
        header={
          shop ? (
            <DialogHeader
              title={shop.name}
              subtitle={
                item ? (
                  <>
                    {shopHost(item.author_uri)}
                    {" · "}
                    <ShopLink name={item.author_name} />
                  </>
                ) : (
                  // the tile has no shop address: the line waits for it, as tall as it will be
                  <span style={{ visibility: "hidden" }}>
                    <ShopLink name={shop.name} />
                  </span>
                )
              }
              startContent={
                <Avatar
                  src={shop.icon || undefined}
                  name={shop.name}
                  size="md"
                  tooltip={false}
                />
              }
              onOpenChange={() => close()}
            />
          ) : (
            <DialogHeader
              // blank lines keep the loaded header's height (title and subtitle)
              title={"\u00a0"}
              subtitle={"\u00a0"}
              startContent={<HeaderSilhouette shown={shown} />}
              onOpenChange={() => close()}
            />
          )
        }
        content={
          // the dialog blocks' default inset (DialogFullscreenDialog.tsx:71), the header's and the footer's: one
          // edge for all three, where the page template's padding 6 put the body inside them
          <LayoutContent ref={body}>
            {item ? (
              <VStack gap={6}>
                <Grid {...COLUMNS} gap={5}>
                  <ItemGallery
                    key={item.id}
                    item={item}
                    isWide={!phone}
                    isOpen={id > 0}
                  />
                  <ItemInfo
                    key={item.id}
                    item={item}
                    rates={known}
                    isWide={!phone}
                  />
                </Grid>
                {/* the sections end on their own Divider (template page.tsx:269) */}
                <MoreFrom
                  key={item.id}
                  item={item}
                  rates={known}
                  isWide={!phone}
                />
              </VStack>
            ) : (
              <DetailSilhouette
                shown={shown}
                columns={COLUMNS}
                isWide={!phone}
                tile={tile}
              />
            )}
          </LayoutContent>
        }
        footer={
          phone ? (
            <LayoutFooter>
              {item ? (
                <PhoneBar item={item} rates={known} />
              ) : (
                <PhoneBarSilhouette shown={shown} price={tile?.price} />
              )}
            </LayoutFooter>
          ) : undefined
        }
      />
    </Dialog>
  );
}

/** "All items from shop": searches the shop's name, as master did. */
// A link in the subtitle (DialogHeader allows inline links there; the title must stay non-interactive), so it
// reads as part of the shop's name, not as dialog chrome beside the close button.
function ShopLink({ name }: { name: string }) {
  const { t } = useUi();
  const { close } = useItemOpener();
  const { apply } = useFilterUrl();
  return (
    <Link
      onClick={() => close(() => apply({ author: name, q: "", avatar: "" }))}
    >
      {t("detail.shopAll")}
    </Link>
  );
}
