// Butter (@astryxdesign/theme-butter), loaded only when chosen or previewed (themes/index.tsx).
import "@astryxdesign/theme-butter/theme.css";
import "./butter.css";

export { butterTheme as theme } from "@astryxdesign/theme-butter/built";
export { default } from "./sheet";

/** Its faces (butter.css), loaded before a switch shows the theme. */
export const fonts = ['1em "Outfit"', '400 1em "Sarina"'];
