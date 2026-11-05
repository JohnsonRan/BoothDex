// The site's own glyphs. Every icon is lucide, the neutral theme's set: its registry maps all of Astryx's
// semantic names (close, chevrons, check, search, ...) to lucide, and these add the few the site needs beyond
// them under the `site:` namespace. Registered once, before anything renders (providers.tsx imports this), so
// any component, server ones included, names them as a string (`<Icon icon="site:favourite" />`) and Astryx's Icon
// sizes and colours them. Glyphs of surfaces that load on demand register with them (icons-on-demand.tsx).
// Mapping: docs/TEMPLATE-MAP.md, Icons.

import { registerIcons } from "@astryxdesign/core/Icon";
import {
  Heart,
  Languages,
  LayoutGrid,
  Moon,
  Settings,
  Sun,
  Trash2,
} from "lucide-react";

/** Props of every registered glyph, as the theme's own entries: Icon sets the size through the font size. */
export const glyph = { size: "1em", "aria-hidden": true } as const;

const SITE_ICONS = {
  "site:favourite": <Heart {...glyph} />,
  "site:settings": <Settings {...glyph} />,
  "site:language": <Languages {...glyph} />,
  "site:allAvatars": <LayoutGrid {...glyph} />,
  "site:dark": <Moon {...glyph} />,
  "site:light": <Sun {...glyph} />,
  "site:clear": <Trash2 {...glyph} />,
};

registerIcons(SITE_ICONS);
