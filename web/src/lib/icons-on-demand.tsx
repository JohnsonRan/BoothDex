// Glyphs only the surfaces that load on demand use (account menu, recent view, viewed mark, settings, search
// suggestions), registered with the first of them rather than with the page (lib/icons.tsx holds the page's).
// lucide, as the neutral theme's own set.

import { registerIcons } from "@astryxdesign/core/Icon";
import {
  Code,
  Coins,
  Eye,
  Hash,
  History,
  LayoutDashboard,
  LogOut,
  Monitor,
  Palette,
  Type,
} from "lucide-react";
import { glyph } from "./icons";

registerIcons({
  "site:viewed": <Eye {...glyph} />,
  "site:recent": <History {...glyph} />,
  "site:admin": <LayoutDashboard {...glyph} />,
  "site:signOut": <LogOut {...glyph} />,
  "site:appearance": <Palette {...glyph} />,
  "site:system": <Monitor {...glyph} />,
  "site:itemText": <Type {...glyph} />,
  "site:currency": <Coins {...glyph} />,
  "site:code": <Code {...glyph} />,
  "site:tag": <Hash {...glyph} />,
});
