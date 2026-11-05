"use client";
// A tile's favourite button: the ghost IconButton of the price row, with the heart filled while the item is
// saved (the pressed icon of block ToggleButtonShowcase, on the tile's borderless button) and `aria-pressed`
// for the state. Toggling is optimistic; signing in first, and failures, are explained by the session's toast.

import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { useAccount } from "@/components/account/session";
import { useUi } from "@/components/providers";

export function FavButton({ id }: { id: number }) {
  const { t } = useUi();
  const { favs, toggleFav } = useAccount();
  const saved = !!favs?.has(id);
  return (
    <IconButton
      label={t(saved ? "fav.remove" : "fav.add")}
      aria-pressed={saved}
      variant="ghost"
      size="sm"
      icon={
        <Icon icon={saved ? "site:favourited" : "site:favourite"} size="sm" />
      }
      onClick={() => toggleFav(id)}
    />
  );
}
