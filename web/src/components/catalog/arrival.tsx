"use client";
// The page's entrance. The results come with the frame; for a text search the server streams the frame first and
// the results when the API answers, and a slow answer paints a Skeleton silhouette first (marked PLACEHOLDER).
// Whatever replaces a silhouette the visitor saw then enters once (lib/motion). Content that came with the first
// paint never moves: the LCP cover and the title are painted at full opacity. Filtering, sorting, paging and
// refreshes keep the observer disconnected, so they never replay it.

import { VStack } from "@astryxdesign/core/Stack";
import { type ReactNode, useEffect, useRef } from "react";
import {
  cancelOnReducedMotion,
  enter,
  prefersReducedMotion,
} from "@/lib/motion";

const SILHOUETTE = "[data-placeholder]";

export function Arrival({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const root = ref.current;
    if (!root?.querySelector(SILHOUETTE) || prefersReducedMotion()) return;
    // silhouettes that have been on screen; one that leaves before its first paint lets its content appear as is
    const painted = new WeakSet<Element>();
    let frame = 0;
    const markPainted = () => {
      const now = root.querySelectorAll(SILHOUETTE);
      // the first frame callback runs before the paint that shows them, the second after it
      frame = requestAnimationFrame(() => {
        frame = requestAnimationFrame(() => {
          for (const el of now) painted.add(el);
        });
      });
    };
    const running: Animation[] = [];
    const stop = cancelOnReducedMotion(running);
    // A streamed boundary swaps its fallback for the content in one script; the observer's callback is a
    // microtask, so the entrance starts before that content is painted. The content takes the silhouette's
    // place in its parent; what the same commit adds elsewhere was on screen already (at hydration, the sort
    // button swapped for its client twin while the results' boundary renders its silhouette again).
    const watch = new MutationObserver((records) => {
      const replaced = new Set<Node>();
      for (const r of records)
        for (const n of r.removedNodes)
          if (n instanceof Element && painted.has(n)) replaced.add(r.target);
      const arrived: Element[] = [];
      for (const r of records)
        if (replaced.has(r.target))
          for (const n of r.addedNodes)
            if (n instanceof Element && !n.matches(SILHOUETTE)) arrived.push(n);
      if (arrived.length && !prefersReducedMotion())
        running.push(...enter(arrived));
      if (root.querySelector(SILHOUETTE)) markPainted();
      else watch.disconnect();
    });
    watch.observe(root, { childList: true, subtree: true });
    markPainted();
    return () => {
      cancelAnimationFrame(frame);
      watch.disconnect();
      stop();
    };
  }, []);
  return (
    <VStack ref={ref} style={{ display: "contents" }}>
      {children}
    </VStack>
  );
}
