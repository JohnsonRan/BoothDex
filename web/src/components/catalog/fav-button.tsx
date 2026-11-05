"use client";
// The tile's favourite button. Accounts come with the sign-in step of the rewrite; until then every visitor is
// signed out, and the click explains why nothing was saved (master's toast), as in block useToastHookUsage.

import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { useToast } from "@astryxdesign/core/Toast";
import { Heart } from "lucide-react";
import { useUi } from "@/components/providers";

export function FavButton() {
  const { t } = useUi();
  const toast = useToast();
  return (
    <IconButton
      label={t("fav.add")}
      variant="ghost"
      size="sm"
      icon={<Icon icon={Heart} size="sm" />}
      // one toast however often it is clicked
      onClick={() => toast({ body: t("fav.needSignIn"), uniqueID: "fav" })}
    />
  );
}
