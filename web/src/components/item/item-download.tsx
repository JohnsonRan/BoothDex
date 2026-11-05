"use client";
// Download of the current image (master's DownloadMenu): a ghost IconButton opening a DropdownMenu of formats, as
// the top bar's menus are built (header-controls.tsx); a failure is an error Toast.

import {
  DropdownMenu,
  DropdownMenuGroup,
  DropdownMenuItem,
} from "@astryxdesign/core/DropdownMenu";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { useToast } from "@astryxdesign/core/Toast";
import { useUi } from "@/components/providers";
import type { ItemResponse } from "@/lib/api-types";
import { imgUrl } from "@/lib/catalog";
import {
  downloadFormats,
  downloadName,
  downloadUrl,
  type Format,
} from "./item-data";

/** The current image at full size, re-encoded by the image proxy (master's DownloadMenu). */
export function Download({
  item,
  index,
}: {
  item: ItemResponse;
  index: number;
}) {
  const { t } = useUi();
  const toast = useToast();
  const orig = imgUrl(item, index, "orig");
  if (!orig) return null;
  const save = async (format: Format) => {
    try {
      const res = await fetch(downloadUrl(orig, format));
      if (!res.ok) throw new Error(String(res.status));
      const a = document.createElement("a");
      a.href = URL.createObjectURL(await res.blob());
      a.download = downloadName(item.id, index, format);
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 10_000);
    } catch {
      toast({
        body: t("list.loadFailed"),
        type: "error",
        uniqueID: "download",
      });
    }
  };
  return (
    <DropdownMenu
      placement="below"
      alignment="start"
      renderTrigger={(props) => (
        <IconButton
          {...props}
          label={t("detail.download")}
          tooltip={t("detail.download")}
          variant="ghost"
          icon={<Icon icon="site:download" size="sm" />}
        />
      )}
    >
      <DropdownMenuGroup title={t("detail.download")}>
        {downloadFormats(orig).map((f) => (
          <DropdownMenuItem
            key={f}
            label={t("detail.downloadAs", { format: f.toUpperCase() })}
            onClick={() => save(f)}
          />
        ))}
      </DropdownMenuGroup>
    </DropdownMenu>
  );
}
