import type { Metadata, Viewport } from "next";
import { Figtree } from "next/font/google";
import "./globals.css";

// The neutral theme's Latin face, self-hosted and preloaded; globals.css points the theme's font tokens at it.
const figtree = Figtree({
  subsets: ["latin"],
  display: "swap",
  variable: "--font-figtree",
});

export const metadata: Metadata = {
  title: "BoothDex",
  description: "Search and filter VRChat assets on Booth",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
};

// Applies the saved appearance and language before first paint (from the cookie, or master's localStorage copy
// until providers.tsx moves it over). Without a saved mode the page follows the system (no data-theme), so only a
// pinned light or dark mode needs the attribute.
const preferences = `try{var m=document.cookie.match(/(?:^|; )settings=([^;]*)/);
var s=JSON.parse(m?decodeURIComponent(m[1]):localStorage.getItem("settings")||"{}");var d=document.documentElement;
if(s.theme==="light"||s.theme==="dark")d.setAttribute("data-theme",s.theme);
var l={zh:"zh-CN",en:"en-US",ja:"ja-JP",ko:"ko-KR"}[s.ui];if(l)d.lang=l}catch(e){}`;

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    // data-theme and lang may be changed by the script above before hydration
    <html
      lang="zh-CN"
      data-astryx-theme="neutral"
      className={figtree.variable}
      suppressHydrationWarning
    >
      <head>
        {/* the site has its own dark mode (Astryx theme, color-scheme set): Dark Reader must not filter it again */}
        <meta name="darkreader-lock" />
        <script
          // biome-ignore lint/security/noDangerouslySetInnerHtml: static inline script, no user input
          dangerouslySetInnerHTML={{ __html: preferences }}
        />
      </head>
      <body>{children}</body>
    </html>
  );
}
