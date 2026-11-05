"use client";
// The session's toasts (session.tsx), in their own chunk so the page loads the toast code with the first
// notice. One toast per message: raising it again replaces it (block useToastHookUsage, `uniqueID`).

import { useToast } from "@astryxdesign/core/Toast";
import { useEffect } from "react";
import { useUi } from "@/components/providers";
import type { Notice } from "./session";

export default function Toaster({ notice }: { notice: Notice }) {
  const { t } = useUi();
  const toast = useToast();
  // biome-ignore lint/correctness/useExhaustiveDependencies: each notice raises it once
  useEffect(() => {
    toast({ body: t(notice.key), type: notice.type, uniqueID: notice.key });
  }, [notice]);
  return null;
}
