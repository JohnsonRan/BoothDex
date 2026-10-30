import type { Metadata, Viewport } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SettingsProvider } from "@/lib/settings";
import "./globals.css";

const geistSans = Geist({ variable: "--font-geist-sans", subsets: ["latin"] });
const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "Catalogue",
  description: "Search and filter VRChat assets on Booth",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  themeColor: "#0a0a0d",
};

// Applies the saved theme and language before first paint, so light-theme visitors don't see a dark flash.
const preferences = `try{var s=JSON.parse(localStorage.getItem("settings")||"{}");var d=document.documentElement;
if(s.theme==="light")d.classList.remove("dark");
var l={zh:"zh-CN",en:"en",ja:"ja",ko:"ko"}[s.ui];if(l)d.lang=l}catch(e){}`;

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    // class/lang may be changed by the script above before hydration
    <html
      lang="zh-CN"
      className={`dark ${geistSans.variable} ${geistMono.variable} h-full antialiased`}
      suppressHydrationWarning
    >
      <head>
        {/* biome-ignore lint/security/noDangerouslySetInnerHtml: static inline script, no user input */}
        <script dangerouslySetInnerHTML={{ __html: preferences }} />
      </head>
      <body className="min-h-full flex flex-col">
        <SettingsProvider>
          <TooltipProvider>{children}</TooltipProvider>
        </SettingsProvider>
        <Toaster />
      </body>
    </html>
  );
}
