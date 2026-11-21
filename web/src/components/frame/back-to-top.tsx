"use client";
// The floating way back up, mounted once in the page frame. Shown after the window has scrolled past one viewport.
// An IntersectionObserver on a viewport-tall marker at the document's top answers that when the browser's own
// scrolling crosses it: no scroll handler, and a state change only at the crossing. (band.tsx and
// use-result-frame.ts listen to scroll because they need the offset itself; this needs one bit.)
// While it shows, the toasts sit above it, as far above its top as it sits above the window's bottom edge
// (toasts.tsx `toastClearance`), so a toast never covers it.

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { useEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import { toastClearance } from "@/components/toasts";
import { prefersReducedMotion } from "@/lib/motion";

/** Where toasts sit above a button at `rect` in a window `height` tall: its top, plus its own gap to the bottom edge. */
export const clearanceAbove = (
  rect: { top: number; bottom: number },
  height: number,
) => 2 * height - rect.top - rect.bottom;

export function BackToTop() {
  const { t } = useUi();
  const marker = useRef<HTMLSpanElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const [away, setAway] = useState(false);
  useEffect(() => {
    const el = marker.current;
    if (!el) return;
    const seen = new IntersectionObserver(([e]) => setAway(!e.isIntersecting));
    seen.observe(el);
    return () => seen.disconnect();
  }, []);
  useEffect(() => {
    const el = button.current;
    if (!away || !el) {
      toastClearance.set(0);
      return;
    }
    const place = () =>
      toastClearance.set(
        clearanceAbove(el.getBoundingClientRect(), innerHeight),
      );
    place();
    // the window's height and the root font size (which sizes the button) change with a resize
    addEventListener("resize", place);
    return () => {
      removeEventListener("resize", place);
      toastClearance.set(0);
    };
  }, [away]);
  const up = () => {
    // the button leaves at the top, so focus moves to the logo link first instead of dropping to the body
    document
      .querySelector<HTMLElement>("a.site-heading")
      ?.focus({ preventScroll: true });
    scrollTo({
      top: 0,
      behavior: prefersReducedMotion() ? "instant" : "smooth",
    });
  };
  return (
    <>
      <span
        ref={marker}
        aria-hidden="true"
        style={{
          position: "absolute",
          insetBlockStart: 0,
          insetInlineStart: 0,
          blockSize: "100vh",
          inlineSize: "1px",
          pointerEvents: "none",
        }}
      />
      {away && (
        <Button
          ref={button}
          label={t("nav.top")}
          tooltip={t("nav.top")}
          icon={<Icon icon="arrowUp" />}
          isIconOnly
          elevation="med"
          onClick={up}
          style={{
            position: "fixed",
            insetInlineEnd:
              "max(var(--spacing-4), env(safe-area-inset-right, 0px))",
            insetBlockEnd:
              "calc(var(--spacing-4) + env(safe-area-inset-bottom, 0px))",
          }}
        />
      )}
    </>
  );
}
