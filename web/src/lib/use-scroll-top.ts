"use client";
// The site's dialogs stay mounted once opened (for their closing transition and a quick reopen), so a scroller
// in one keeps its offset: opened again, or showing another item or panel, it started where the last one was
// left. This puts it back at its top before the frame is painted.

import { useLayoutEffect, useRef } from "react";

/** A ref for a scroller that returns to its top whenever `view` changes to a truthy value (falsy: closed). */
export function useScrollTop<T extends HTMLElement = HTMLDivElement>(
  view: unknown,
) {
  const ref = useRef<T>(null);
  useLayoutEffect(() => {
    if (view) ref.current?.scrollTo({ top: 0, behavior: "instant" });
  }, [view]);
  return ref;
}
