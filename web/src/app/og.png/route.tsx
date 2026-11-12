import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { ImageResponse } from "next/og";
import { SHARE_IMAGE } from "@/lib/seo";

// The home page's link preview picture (SHARE_IMAGE in lib/seo.ts), drawn once at build time: the mark and name,
// what the site holds, its search box and its main categories.
// It is a route, not the opengraph-image convention: Next resolves that file against the root layout, which has
// no metadataBase because SITE_URL is read per request, so every page but the home page linked it on localhost.
// next/og lays out plain elements with inline styles only, so this file cannot use Astryx components or tokens:
// the colours below are neutral's, named after their tokens.

// the site's typeface (layout.tsx); next/og reads WOFF but not WOFF2, which next/font serves
const font = (weight: 500 | 700) =>
  readFile(
    join(
      process.cwd(),
      `node_modules/@fontsource/figtree/files/figtree-latin-${weight}-normal.woff`,
    ),
  );
const [medium, bold] = await Promise.all([font(500), font(700)]);

// light mode: --color-background-surface, --color-text-primary, --color-text-secondary, --color-background-muted
// and --color-border
const SURFACE = "#ffffff";
const TEXT = "#000000";
const SECONDARY = "#474747";
const MUTED = "#f1f1f1";
const BORDER = "#00000014";

// dark mode's --color-border-<colour>: light mode's are pastels that fade into the white
const CATEGORIES = [
  ["Avatars", "#df843f"],
  ["Clothing", "#fa6762"],
  ["Hair", "#5aa0f8"],
  ["Accessories", "#d885eb"],
  ["Tools", "#4fb1a0"],
] as const;

export function GET() {
  return new ImageResponse(
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        background: SURFACE,
        color: TEXT,
        fontFamily: "Figtree",
        fontWeight: 500,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 28 }}>
        {/* the BoothDex mark, as components/frame/logo.tsx; the picture's text is SHARE_IMAGE.alt */}
        <svg
          viewBox="0 0 32 32"
          width={104}
          height={104}
          fill={TEXT}
          aria-hidden="true"
        >
          <mask id="m">
            <rect width="32" height="32" fill="#fff" />
            <path
              d="M8 10v12M8 10h2a3 3 0 0 1 0 6H8M8 16h2.5a3 3 0 0 1 0 6H8M19 10v12M19 10h.5a6 6 0 0 1 0 12H19"
              fill="none"
              stroke="#000"
              strokeWidth="3.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </mask>
          <path
            mask="url(#m)"
            d="M8 3h16a6 6 0 0 1 6 6v12a6 6 0 0 1-6 6H13l-6 4v-4.3A6 6 0 0 1 2 21V9a6 6 0 0 1 6-6Z"
          />
        </svg>
        <div style={{ fontSize: 104, fontWeight: 700, letterSpacing: -2 }}>
          BoothDex
        </div>
      </div>
      <div style={{ marginTop: 18, fontSize: 34, color: SECONDARY }}>
        VRChat avatars, clothing, accessories and tools on BOOTH
      </div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 22,
          width: 920,
          height: 92,
          marginTop: 52,
          padding: "0 32px",
          borderRadius: 20,
          border: `2px solid ${BORDER}`,
          background: MUTED,
          fontSize: 34,
          color: SECONDARY,
        }}
      >
        {/* lucide's search icon, as the site's search box */}
        <svg
          viewBox="0 0 24 24"
          width={38}
          height={38}
          fill="none"
          stroke={SECONDARY}
          strokeWidth="2"
          strokeLinecap="round"
          aria-hidden="true"
        >
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.3-4.3" />
        </svg>
        Search by avatar, category and price…
      </div>
      <div
        style={{
          display: "flex",
          gap: 40,
          marginTop: 36,
          fontSize: 30,
          color: SECONDARY,
        }}
      >
        {CATEGORIES.map(([name, dot]) => (
          <div
            key={name}
            style={{ display: "flex", alignItems: "center", gap: 14 }}
          >
            <div
              style={{
                width: 16,
                height: 16,
                borderRadius: 8,
                background: dot,
              }}
            />
            {name}
          </div>
        ))}
      </div>
    </div>,
    {
      width: SHARE_IMAGE.width,
      height: SHARE_IMAGE.height,
      fonts: [
        { name: "Figtree", data: medium, weight: 500 },
        { name: "Figtree", data: bold, weight: 700 },
      ],
    },
  );
}
