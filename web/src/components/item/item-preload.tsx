"use client";
// A shared link (`?item=`) paints the dialog's main image largest, but the dialog renders in the browser: its
// code loads, the item is fetched, and only then is the image asked for. The server already knows the item, so
// the first HTML asks for the image (a resource hint, Next's generate-metadata guide) and it is there when the
// dialog renders. `src` is the address the gallery gives that image (item-gallery.tsx).

import ReactDOM from "react-dom";

export function PreloadHero({ src }: { src: string }) {
  ReactDOM.preload(src, { as: "image", fetchPriority: "high" });
  return null;
}
