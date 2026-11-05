import type { Metadata, Viewport } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "BoothDex",
  description: "Search and filter VRChat assets on Booth",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
};

// Applies the saved appearance and language before first paint. Without a saved mode the page follows the
// system (no data-theme), so only a pinned light or dark mode needs the attribute.
const preferences = `try{var m=document.cookie.match(/(?:^|; )settings=([^;]*)/);
var s=JSON.parse(m?decodeURIComponent(m[1]):"{}");var d=document.documentElement;
if(s.theme==="light"||s.theme==="dark")d.setAttribute("data-theme",s.theme);
var l={zh:"zh-CN",en:"en-US",ja:"ja-JP",ko:"ko-KR"}[s.ui];if(l)d.lang=l}catch(e){}`;

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    // data-theme and lang may be changed by the script above before hydration
    <html lang="zh-CN" data-astryx-theme="neutral" suppressHydrationWarning>
      <head>
        <script
          // biome-ignore lint/security/noDangerouslySetInnerHtml: static inline script, no user input
          dangerouslySetInnerHTML={{ __html: preferences }}
        />
      </head>
      <body>{children}</body>
    </html>
  );
}
