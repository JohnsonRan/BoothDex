"use client";
// Master's "viewed" treatment (item-card.tsx:72-79) drawn with Astryx: a light scrim over the cover with a
// "Viewed" Badge in it, as block OverlayBottomStrip lays a Badge over media with Overlay. As on master, a
// mouse over the tile lifts the scrim so the cover can be seen. Only a viewed tile mounts the Overlay, whose
// code loads with the first one; the others render their cover as given.

import { Badge } from "@astryxdesign/core/Badge";
import { Icon } from "@astryxdesign/core/Icon";
import { Overlay } from "@astryxdesign/core/Overlay";
import { useState } from "react";
import { useUi } from "@/components/providers";
import "@/lib/icons-on-demand";

/** The scrim and badge over one viewed cover (loaded on demand by viewed-mark.tsx). */
export default function ViewedOverlay({
  children,
}: {
  children: React.ReactNode;
}) {
  const { t } = useUi();
  const [hover, setHover] = useState(false);
  return (
    <Overlay
      scrim="light"
      position="fill"
      align="start"
      isOpen={!hover}
      onPointerEnter={(e) => setHover(e.pointerType === "mouse")}
      onPointerLeave={() => setHover(false)}
      content={
        <Badge
          label={t("viewed.badge")}
          icon={<Icon icon="site:viewed" size="xsm" />}
        />
      }
    >
      {children}
    </Overlay>
  );
}
