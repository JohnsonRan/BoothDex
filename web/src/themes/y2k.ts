// Y2K (@astryxdesign/theme-y2k), loaded only when chosen or previewed (themes/index.tsx).
import "@astryxdesign/theme-y2k/theme.css";
import "./y2k.css";

export { y2kTheme as theme } from "@astryxdesign/theme-y2k/built";
export { default } from "./sheet";

/** Its faces (y2k.css), loaded before a switch shows the theme. */
export const fonts = [
  '400 1em "Poppins"',
  '500 1em "Poppins"',
  '600 1em "Poppins"',
  '700 1em "Poppins"',
  '400 1em "Crimson Text"',
];
