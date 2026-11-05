// Glyphs only the surfaces that load on demand use (account menu, saved searches, recent view, viewed mark,
// settings, palette), registered with the first of them rather than with the page (lib/icons.tsx holds the
// page's). lucide, as the neutral theme's own set.

import { registerIcons } from "@astryxdesign/core/Icon";
import {
  Bookmark,
  Code,
  Coins,
  Eye,
  Hash,
  History,
  LayoutDashboard,
  LogOut,
  Monitor,
  Palette,
  Rss,
  Type,
} from "lucide-react";

const glyph = { size: "1em", "aria-hidden": true } as const;

registerIcons({
  "site:viewed": <Eye {...glyph} />,
  "site:recent": <History {...glyph} />,
  "site:savedSearch": <Rss {...glyph} />,
  "site:subscribe": <Bookmark {...glyph} />,
  "site:admin": <LayoutDashboard {...glyph} />,
  "site:signOut": <LogOut {...glyph} />,
  "site:appearance": <Palette {...glyph} />,
  "site:system": <Monitor {...glyph} />,
  "site:itemText": <Type {...glyph} />,
  "site:currency": <Coins {...glyph} />,
  "site:code": <Code {...glyph} />,
  "site:tag": <Hash {...glyph} />,
});
