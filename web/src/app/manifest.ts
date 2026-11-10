import type { MetadataRoute } from "next";

// Members Next's type does not list yet. `description_localized` is read by Chrome and Edge 148 on desktop;
// `color_scheme_dark` is specified but no browser implements it, so until one does an installed app takes its
// title bar from the page's light and dark theme-color (layout.tsx viewport), and only the splash stays light.
type Manifest = MetadataRoute.Manifest & {
  description_localized?: Record<string, string>;
  color_scheme_dark?: { theme_color?: string; background_color?: string };
};

// neutral's --color-background-surface per system mode, as layout.tsx's theme-color
const LIGHT = "#ffffff";
const DARK = "#262626";

export default function manifest(): Manifest {
  return {
    id: "/",
    name: "BoothDex",
    short_name: "BoothDex",
    lang: "en",
    dir: "ltr",
    description: "Search and filter VRChat assets on Booth",
    description_localized: {
      zh: "搜索和筛选 Booth 上的 VRChat 素材",
      ja: "Booth の VRChat アセットを検索して絞り込めます",
      ko: "Booth의 VRChat 에셋을 검색하고 필터링합니다",
    },
    start_url: "/",
    scope: "/",
    display: "standalone",
    background_color: LIGHT,
    theme_color: LIGHT,
    color_scheme_dark: { background_color: DARK, theme_color: DARK },
    icons: [
      {
        src: "/icon-192.png",
        sizes: "192x192",
        type: "image/png",
      },
      {
        src: "/icon-512.png",
        sizes: "512x512",
        type: "image/png",
      },
      // the logo inside the central 80 % circle on a full white square, so a launcher's mask never cuts it
      {
        src: "/icon-maskable-512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "maskable",
      },
    ],
  };
}
