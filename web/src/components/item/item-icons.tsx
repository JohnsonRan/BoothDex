// Glyphs only the item dialog uses, registered with its chunk rather than with the page (lib/icons.tsx holds
// the page's). lucide, as the neutral theme's own set; the dialog's other icons are the theme's semantic names
// (chevronLeft, chevronRight, copy, externalLink).

import { registerIcons } from "@astryxdesign/core/Icon";
import { ArrowRight, Download } from "lucide-react";

const glyph = { size: "1em", "aria-hidden": true } as const;

registerIcons({
  "site:download": <Download {...glyph} />,
  "site:arrowRight": <ArrowRight {...glyph} />,
});
