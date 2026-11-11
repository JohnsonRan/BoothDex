// A session's device as people name it ("Chrome · Windows"), from the user agent the API stored at sign-in.
// Order matters: Edge and Opera also say Chrome, Chrome also says Safari, Android also says Linux.

/** A live session (crates/api/src/account.rs sessions_json); `current` is the one asking. */
export type AccountSession = {
  id: number;
  created_at: string;
  expires_at: string;
  user_agent: string | null;
  ip: string | null;
  current: boolean;
};

const BROWSERS: [RegExp, string][] = [
  [/Edg(e|A|iOS)?\//, "Edge"],
  [/OPR\/|Opera/, "Opera"],
  [/Firefox\/|FxiOS/, "Firefox"],
  [/Chrome\/|CriOS/, "Chrome"],
  [/Safari\//, "Safari"],
];
const SYSTEMS: [RegExp, string][] = [
  [/Windows/, "Windows"],
  [/iPhone|iPad|iPod/, "iOS"],
  [/Android/, "Android"],
  [/Mac OS X|Macintosh/, "macOS"],
  [/CrOS/, "ChromeOS"],
  [/Linux/, "Linux"],
];

const first = (ua: string, list: [RegExp, string][]) =>
  list.find(([re]) => re.test(ua))?.[1];

/** The browser and system, or null when the user agent names neither. */
export function deviceName(ua: string | null): string | null {
  if (!ua) return null;
  const parts = [first(ua, BROWSERS), first(ua, SYSTEMS)].filter(Boolean);
  return parts.length ? parts.join(" · ") : null;
}
