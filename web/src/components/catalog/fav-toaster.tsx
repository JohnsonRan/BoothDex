"use client";
// The toast behind fav-toast.tsx, in its own chunk so the page does not load the toast code until a click.

import { useToast } from "@astryxdesign/core/Toast";
import { useEffect } from "react";
import { useUi } from "@/components/providers";

export default function FavToaster({ clicks }: { clicks: number }) {
  const { t } = useUi();
  const toast = useToast();
  // biome-ignore lint/correctness/useExhaustiveDependencies: each click raises it
  useEffect(() => {
    toast({ body: t("fav.needSignIn"), uniqueID: "fav" });
  }, [clicks]);
  return null;
}
