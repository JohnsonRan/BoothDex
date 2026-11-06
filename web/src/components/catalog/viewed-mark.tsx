"use client";
// Whether a tile's cover carries the "viewed" mark (viewed-overlay.tsx): only for items in the session's list.
// The Overlay's code loads with the first viewed tile; until then the cover shows as is.

import { lazy, Suspense } from "react";
import { useAccount } from "@/components/account/session";

const ViewedOverlay = lazy(() => import("./viewed-overlay"));

export function ViewedMark({
  id,
  children,
}: {
  id: number;
  children: React.ReactNode;
}) {
  const { viewed } = useAccount();
  if (!viewed?.includes(id)) return children;
  return (
    <Suspense fallback={children}>
      <ViewedOverlay>{children}</ViewedOverlay>
    </Suspense>
  );
}
