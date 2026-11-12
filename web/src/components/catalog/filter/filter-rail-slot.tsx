"use client";
// The filters' rail, loaded only for a visitor who puts the filters at the side: home-screen.tsx is a Server
// Component, whose imports of Client Components are not split (Next.js guides/lazy-loading), so the rail's code
// (its lists, sections and switches) went to every visitor. `next/dynamic` from a Client Component keeps it in a
// chunk of its own; the server still renders it, so the page does not move when it hydrates.

import dynamic from "next/dynamic";

export const FilterRail = dynamic(() =>
  import("./filter-rail").then((m) => m.FilterRail),
);
