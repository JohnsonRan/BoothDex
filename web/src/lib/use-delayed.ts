"use client";
// The loading rule (doona apps/lite UI-RULES §2): the layout is reserved at once, a loading indicator shows only
// once a wait has lasted 150 ms, and once shown it stays at least 300 ms, so a quick answer shows nothing and a
// slow one never flickers.

import { useEffect, useLayoutEffect, useRef, useState } from "react";

export const SHOW_AFTER = 150;
export const HOLD_FOR = 300;

/** Whether the indicator for `waiting` is visible now. */
export function useDelayed(waiting: boolean): boolean {
  const [visible, setVisible] = useState(false);
  const shownAt = useRef(0);
  // painted while still waiting: a busy main thread can run the timer only as the answer commits, and an
  // indicator first drawn after the answer would only flash
  const painted = useRef(false);
  // the hold counts from the commit, not from the timer
  useLayoutEffect(() => {
    if (!visible) return;
    shownAt.current = performance.now();
    painted.current = false;
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => {
        painted.current = true;
      });
    });
    return () => cancelAnimationFrame(frame);
  }, [visible]);
  useEffect(() => {
    if (waiting) {
      if (visible) return;
      const timer = setTimeout(() => setVisible(true), SHOW_AFTER);
      return () => clearTimeout(timer);
    }
    if (!visible) return;
    if (!painted.current) return setVisible(false);
    const left = HOLD_FOR - (performance.now() - shownAt.current);
    const timer = setTimeout(() => setVisible(false), Math.max(0, left));
    return () => clearTimeout(timer);
  }, [waiting, visible]);
  // the answer may commit together with the timer's update: then it never shows at all
  return visible && (waiting || painted.current);
}
