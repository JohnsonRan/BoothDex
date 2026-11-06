import type { Metadata, Viewport } from "next";
import { Figtree } from "next/font/google";
import { EARLY_CLICKS } from "@/lib/early-clicks";
import { SESSION_HINT_SCRIPT } from "@/lib/session-hint";
import { SINGLE_MODE, STYLES } from "@/lib/settings-core";
import "./globals.css";
import "./filter-row.css";

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

// The phone browser's bar takes the page's background: neutral's --color-background-surface, the site AppShell's
// (site-frame.tsx), for each system mode; a pinned mode or another theme rewrites it (providers.tsx showChrome).
// Safari 26 on iOS ignores theme-color: it tints its bars from <html>'s background (globals.css), and only with
// viewport-fit=cover does the page run on under the bottom toolbar instead of leaving a solid bar there.
export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#ffffff" },
    { media: "(prefers-color-scheme: dark)", color: "#262626" },
  ],
};

// Applies the saved appearance and language before first paint (from the cookie, or master's localStorage copy
// until providers.tsx moves it over). Without a saved mode the page follows the system (no data-theme), so only a
// pinned light or dark mode, or a single-mode theme's (SINGLE_MODE), needs the attribute; a theme other than
// neutral marks <html> for the page's background (the server renders the page itself in it, themes/index.tsx), and
// pure black marks it with data-black (globals.css).
// SESSION_HINT_SCRIPT marks the last session's sign-in the same way.
const preferences = `try{var m=document.cookie.match(/(?:^|; )settings=([^;]*)/);
var s=JSON.parse(m?decodeURIComponent(m[1]):localStorage.getItem("settings")||"{}");var d=document.documentElement;
s.theme=${JSON.stringify(SINGLE_MODE)}[s.style]||s.theme;
if(s.theme==="light"||s.theme==="dark")d.setAttribute("data-theme",s.theme);
if(/^(${STYLES.join("|")})$/.test(s.style))d.setAttribute("data-astryx-theme",s.style);
if(s.black==="on")d.setAttribute("data-black","");
var l={zh:"zh-CN",en:"en-US",ja:"ja-JP",ko:"ko-KR"}[s.ui];if(l)d.lang=l}catch(e){}`;

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    // data-theme, data-astryx-theme, data-black, data-account and lang may be changed by the script below before hydration
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
          dangerouslySetInnerHTML={{
            __html: preferences + SESSION_HINT_SCRIPT + EARLY_CLICKS,
          }}
        />
      </head>
      <body>{children}</body>
    </html>
  );
}
