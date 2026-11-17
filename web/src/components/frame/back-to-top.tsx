"use client";
// The floating way back up, mounted once in the page frame. Shown after the window has scrolled past one viewport.
// An IntersectionObserver on a viewport-tall marker at the document's top answers that when the browser's own
// scrolling crosses it: no scroll handler, and a state change only at the crossing. (band.tsx and
// use-result-frame.ts listen to scroll because they need the offset itself; this needs one bit.)

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { useEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import { prefersReducedMotion } from "@/lib/motion";

export function BackToTop() {
  const { t } = useUi();
  const marker = useRef<HTMLSpanElement>(null);
  const [away, setAway] = useState(false);
  useEffect(() => {
    const el = marker.current;
    if (!el) return;
    const seen = new IntersectionObserver(([e]) => setAway(!e.isIntersecting));
    seen.observe(el);
    return () => seen.disconnect();
  }, []);
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
