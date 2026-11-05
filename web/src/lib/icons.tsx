// The site's own glyphs. Every icon is lucide, the neutral theme's set: its registry maps all of Astryx's
// semantic names (close, chevrons, check, search, ...) to lucide, and these add the few the site needs beyond
// them under the `site:` namespace. Registered once, before anything renders (providers.tsx imports this), so
// any component, server ones included, names them as a string (`<Icon icon="site:favourite" />`) and Astryx's Icon
// sizes and colours them. Mapping: docs/TEMPLATE-MAP.md, Icons.

import { registerIcons } from "@astryxdesign/core/Icon";
import {
  Bookmark,
  Code,
  Coins,
  Eye,
  Hash,
  Heart,
  History,
  Languages,
  LayoutDashboard,
  LayoutGrid,
  LogOut,
  Monitor,
  Moon,
  Palette,
  Rss,
  Settings,
  Sun,
  Trash2,
  Type,
} from "lucide-react";

// as the theme's own entries: Icon sets the size through the font size
const glyph = { size: "1em", "aria-hidden": true } as const;

const SITE_ICONS = {
  "site:favourite": <Heart {...glyph} />,
  "site:viewed": <Eye {...glyph} />,
  "site:recent": <History {...glyph} />,
  "site:savedSearch": <Rss {...glyph} />,
  "site:subscribe": <Bookmark {...glyph} />,
  "site:admin": <LayoutDashboard {...glyph} />,
  "site:settings": <Settings {...glyph} />,
  "site:signOut": <LogOut {...glyph} />,
  "site:appearance": <Palette {...glyph} />,
  "site:system": <Monitor {...glyph} />,
  "site:itemText": <Type {...glyph} />,
  "site:currency": <Coins {...glyph} />,
  "site:code": <Code {...glyph} />,
  "site:language": <Languages {...glyph} />,
  "site:allAvatars": <LayoutGrid {...glyph} />,
  "site:dark": <Moon {...glyph} />,
  "site:light": <Sun {...glyph} />,
  "site:tag": <Hash {...glyph} />,
  "site:clear": <Trash2 {...glyph} />,
};

registerIcons(SITE_ICONS);
