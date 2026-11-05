import type { Metadata, Viewport } from "next";
import { Geist_Mono } from "next/font/google";
// MiSans variable font in unicode-range slices: the browser downloads only the slices a page uses.
// No CJK web-font fallback: the browser fetches it too while MiSans loads; the few missing glyphs use system fonts
import "subsetted-fonts/MiSans-VF/MiSans-VF.css";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import "./globals.css";

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
  // only the admin console uses it; don't make every visitor download it up front
  preload: false,
});

export const metadata: Metadata = {
  title: "BoothDex",
  description: "Search and filter VRChat assets on Booth",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  themeColor: "#1b1714",
};

// Applies the saved theme and language before first paint, so light-theme visitors don't see a dark flash.
const preferences = `try{var m=document.cookie.match(/(?:^|; )settings=([^;]*)/);
var s=JSON.parse(m?decodeURIComponent(m[1]):localStorage.getItem("settings")||"{}");var d=document.documentElement;
if(s.theme==="light")d.classList.remove("dark");
var l={zh:"zh-CN",en:"en",ja:"ja",ko:"ko"}[s.ui];if(l)d.lang=l}catch(e){}`;

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    // class/lang may be changed by the script above before hydration
    <html
      lang="zh-CN"
      className={`dark ${geistMono.variable} h-full antialiased`}
      suppressHydrationWarning
    >
      <head>
        {/* biome-ignore lint/security/noDangerouslySetInnerHtml: static inline script, no user input */}
        <script dangerouslySetInnerHTML={{ __html: preferences }} />
      </head>
      <body className="min-h-full flex flex-col">
        {/* settings are provided per page (see app/page.tsx), where the request cookie is available */}
        <TooltipProvider>{children}</TooltipProvider>
        <Toaster />
      </body>
    </html>
  );
}
