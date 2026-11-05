"use client";
// The pager under the results, loaded once it comes within a screen or two of the viewport: Pagination and
// its page-number field are ~6 KB of script the first screen never shows. Until then an empty row of its
// height holds its place, so nothing below it moves when it arrives.

import { VStack } from "@astryxdesign/core/Stack";
import dynamic from "next/dynamic";
import { useEffect, useRef, useState } from "react";

const PAGER_HEIGHT = "var(--size-element-md)";

const Pager = dynamic(() => import("./pager").then((m) => m.Pager), {
  ssr: false,
  loading: () => <VStack style={{ minBlockSize: PAGER_HEIGHT }} />,
});

export function LazyPager(props: {
  page: number;
  total: number;
  noSemantic: boolean;
}) {
  const [near, setNear] = useState(false);
  const slot = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = slot.current;
    if (!el) return;
    const seen = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) setNear(true);
      },
      { rootMargin: "0px 0px 200% 0px" },
    );
    seen.observe(el);
    return () => seen.disconnect();
  }, []);

  if (props.total <= 1) return null;
  if (near) return <Pager {...props} />;
  return <VStack ref={slot} style={{ minBlockSize: PAGER_HEIGHT }} />;
}
