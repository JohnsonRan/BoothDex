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
import { Divider } from "@astryxdesign/core/Divider";
import { Grid } from "@astryxdesign/core/Grid";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/Stack";
import { useToast } from "@astryxdesign/core/Toast";
import { use, useEffect, useRef, useState } from "react";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { ItemResponse } from "@/lib/api-types";
import { imageSrc } from "@/lib/catalog";
import type { Rates } from "@/lib/settings-core";
import { useDelayed } from "@/lib/use-delayed";
import { shopHost } from "./item-data";
import { ItemGallery } from "./item-gallery";
import { useItemOpener } from "./item-host";
import { ItemInfo, PhoneBar } from "./item-info";
import { DetailSilhouette, HeaderSilhouette } from "./item-silhouette";
import { MoreFrom } from "./more-from";
import "./item-icons";

/** Below lg the dialog is full screen (master's breakpoint). */
const PHONE = "(max-width: 1023px)";
// template: Layout contentWidth={1200}
const WIDTH = 1200;
// master: 92 % of the viewport's height
const MAX_HEIGHT = "92dvh";
// template: Grid columns={{minWidth: 320, repeat: 'fit'}} gap={5}
const COLUMNS = { minWidth: 320, repeat: "fit" } as const;

type Gallery = { images: string[]; complete: boolean };

/** The full record (cards carry no bio, variations or avatars) and, once the API has it, Booth's whole gallery. */
function useItem(id: number): ItemResponse | null {
  const { t } = useUi();
  const toast = useToast();
  const { close } = useItemOpener();
  const [item, setItem] = useState<ItemResponse | null>(null);
  // an unknown or failed item: error toast, parameter removed (master browse.tsx:347-366)
  const failed = useRef(() => {});
  failed.current = () => {
    toast({ body: t("list.loadFailed"), type: "error", uniqueID: "item" });
    close();
  };
  useEffect(() => {
    if (!id) return;
    let live = true;
    let gallery: string[] | null = null;
    const withGallery = (it: ItemResponse): ItemResponse =>
      gallery ? { ...it, booth_images: gallery, media: [] } : it;
    const fail = () => live && failed.current();
    api<{ items: ItemResponse[] }>(`/api/items?ids=${id}`)
      .then((r) =>
        r.items[0] ? live && setItem(withGallery(r.items[0])) : fail(),
      )
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
              title={<HeaderSilhouette shown={shown} />}
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
                  <ItemGallery key={item.id} item={item} />
                  <ItemInfo
                    key={item.id}
                    item={item}
                    rates={known}
                    hasActions={!phone}
                  />
                </Grid>
                <Divider />
                <MoreFrom item={item} rates={known} />
              </VStack>
            ) : (
              <DetailSilhouette shown={shown} columns={COLUMNS} />
            )}
          </LayoutContent>
        }
        footer={
          phone && item ? (
            <LayoutFooter>
              <PhoneBar item={item} rates={known} />
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
