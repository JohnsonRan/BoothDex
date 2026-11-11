// Glyphs only the surfaces that load on demand use (account menu, recent view, viewed mark, settings, search
// suggestions, saved views and their forms, tag editor), registered with the first of them rather than with the
// page (lib/icons.tsx holds the page's).
// lucide, as the neutral theme's own set.

import { registerIcons } from "@astryxdesign/core/Icon";
import {
  Code,
  Coins,
  Eye,
  FileText,
  Hash,
  History,
  LayoutDashboard,
  LogOut,
  MessageSquare,
  Minus,
  Monitor,
  MonitorSmartphone,
  Palette,
  PanelLeft,
  Pencil,
  Plus,
  Rss,
  Shapes,
  ShieldCheck,
  SwatchBook,
  Type,
} from "lucide-react";
import { glyph } from "./icons";

registerIcons({
  "site:viewed": <Eye {...glyph} />,
  "site:recent": <History {...glyph} />,
  "site:admin": <LayoutDashboard {...glyph} />,
  "site:signOut": <LogOut {...glyph} />,
  "site:appearance": <Palette {...glyph} />,
  // the Astryx theme's row: the template's accent row icon (settings-dialog page.tsx:1288, SwatchIcon)
  "site:style": <SwatchBook {...glyph} />,
  "site:system": <Monitor {...glyph} />,
  "site:devices": <MonitorSmartphone {...glyph} />,
  "site:itemText": <Type {...glyph} />,
  // the filters' place (settings/panels.tsx)
  "site:sidebar": <PanelLeft {...glyph} />,
  "site:currency": <Coins {...glyph} />,
  "site:code": <Code {...glyph} />,
  "site:feedback": <MessageSquare {...glyph} />,
  "site:privacy": <ShieldCheck {...glyph} />,
  "site:terms": <FileText {...glyph} />,
  "site:tag": <Hash {...glyph} />,
  // a search word read as a category (catalog/search-suggest.tsx)
  "site:category": <Shapes {...glyph} />,
  "site:edit": <Pencil {...glyph} />,
  "site:feed": <Rss {...glyph} />,
  "site:add": <Plus {...glyph} />,
  "site:remove": <Minus {...glyph} />,
});
