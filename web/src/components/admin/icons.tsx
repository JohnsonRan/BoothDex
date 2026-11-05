// The admin console's glyphs, registered beside the site's (lib/icons.tsx) under the `admin:` namespace, all
// lucide. Imported by the console's root, so they load only with /admin.

import { registerIcons } from "@astryxdesign/core/Icon";
import {
  ArrowLeft,
  CloudOff,
  Copy,
  Inbox,
  LayoutDashboard,
  ListChecks,
  LogIn,
  Play,
  RefreshCw,
  ScrollText,
  Settings,
  ShieldAlert,
  Square,
} from "lucide-react";
import { glyph } from "@/lib/icons";

registerIcons({
  "admin:overview": <LayoutDashboard {...glyph} />,
  "admin:jobs": <ListChecks {...glyph} />,
  "admin:settings": <Settings {...glyph} />,
  "admin:site": <ArrowLeft {...glyph} />,
  "admin:signIn": <LogIn {...glyph} />,
  "admin:forbidden": <ShieldAlert {...glyph} />,
  "admin:offline": <CloudOff {...glyph} />,
  "admin:empty": <Inbox {...glyph} />,
  "admin:run": <Play {...glyph} />,
  "admin:stop": <Square {...glyph} />,
  "admin:log": <ScrollText {...glyph} />,
  "admin:reload": <RefreshCw {...glyph} />,
  "admin:copy": <Copy {...glyph} />,
});
