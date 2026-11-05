"use client";
// The item dialog: page template product-detail (templates/pages/product-detail/page.tsx) inside an Astryx
// Dialog, as blocks DialogFullscreenDialog and DialogAdaptivePresentation compose one: a Layout with a
// DialogHeader, a LayoutContent and a LayoutFooter. A wide standard dialog from lg up, the fullscreen variant
// below it (master: full screen on phones), where the footer carries price, favourite and Buy on Booth.
// The header names the shop (block DialogWithSubtitle: title and subtitle, avatar as start content); the
// dialog itself is labelled by the item's title. Inside, the template's two columns (gallery, info), then
// the "More from" strip across both.

import { Avatar } from "@astryxdesign/core/Avatar";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Grid } from "@astryxdesign/core/Grid";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/Stack";
import { use, useEffect, useRef, useState } from "react";
import { useAccount } from "@/components/account/session";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { ItemResponse } from "@/lib/api-types";
import { imageSrc } from "@/lib/catalog";
import type { Rates } from "@/lib/settings-core";
import { useDelayed } from "@/lib/use-delayed";
import { PhoneBar } from "./item-actions";
import { shopHost } from "./item-data";
import { ItemGallery } from "./item-gallery";
import { useItemOpener } from "./item-host";
import { ItemInfo } from "./item-info";
import {
  DetailSilhouette,
  HeaderSilhouette,
  MAX_HEIGHT,
  PhoneBarSilhouette,
} from "./item-silhouette";
import { MoreFrom } from "./more-from";
import "./item-icons";

/** Below lg the dialog is full screen (master's breakpoint). */
const PHONE = "(max-width: 1023px)";
// template: Layout contentWidth={1200}
const WIDTH = 1200;
// template: Grid columns={{minWidth: 320, repeat: 'fit'}} gap={5}
const COLUMNS = { minWidth: 320, repeat: "fit" } as const;

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
    if (!id) return;
    let live = true;
    let gallery: string[] | null = null;
    const withGallery = (it: ItemResponse): ItemResponse =>
      gallery ? { ...it, booth_images: gallery, media: [] } : it;
    const fail = () => live && failed.current();
    api<{ items: ItemResponse[] }>(`/api/items?ids=${id}`)
      .then((r) => {
        const it = r.items[0];
        if (!it) return fail();
        if (!live) return;
        setItem(withGallery(it));
        viewed.current(id);
      })
      .catch(fail);
    api<Gallery>(`/api/items/${id}/gallery`)
      .then((g) => {
        if (!live || !g.complete || !g.images.length) return;
        gallery = g.images;
        setItem((it) => (it?.id === id ? withGallery(it) : it));
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [id]);
  // while closing (id 0) the last item stays on screen
  return item && (item.id === id || !id) ? item : null;
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

  return (
    <Dialog
      isOpen={id > 0}
      onOpenChange={(open) => !open && close()}
      variant={phone ? "fullscreen" : "standard"}
      width={WIDTH}
      maxHeight={MAX_HEIGHT}
      aria-label={item?.title ?? t("detail.label")}
    >
      <Layout
        header={
          item ? (
            <DialogHeader
              title={item.author_name}
              subtitle={shopHost(item.author_uri)}
              startContent={
                <Avatar
                  src={imageSrc(item.author_icon) || undefined}
                  name={item.author_name}
                  size="md"
                  tooltip={false}
                />
              }
              endContent={<ShopButton name={item.author_name} />}
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
          // template: LayoutContent padding={6}
          <LayoutContent padding={6}>
            {item ? (
              <VStack gap={6}>
                <Grid columns={COLUMNS} gap={5}>
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
                <MoreFrom item={item} rates={known} isWide={!phone} />
              </VStack>
            ) : (
              <DetailSilhouette
                shown={shown}
                columns={COLUMNS}
                isWide={!phone}
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
                <PhoneBarSilhouette shown={shown} />
              )}
            </LayoutFooter>
          ) : undefined
        }
      />
    </Dialog>
  );
}

/** "All items from shop": searches the shop's name, as master did. */
function ShopButton({ name }: { name: string }) {
  const { t } = useUi();
  const { close } = useItemOpener();
  const { apply } = useFilterUrl();
  return (
    <Button
      label={t("detail.shopAll")}
      variant="secondary"
      size="sm"
      onClick={() => close(() => apply({ author: name, q: "", avatar: "" }))}
    />
  );
}
