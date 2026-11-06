// Matcha (@astryxdesign/theme-matcha), loaded only when chosen or previewed (themes/index.tsx).
import "@astryxdesign/theme-matcha/theme.css";
import "./matcha.css";

export { matchaTheme as theme } from "@astryxdesign/theme-matcha/built";
export { default } from "./sheet";

/** Its faces (matcha.css), loaded before a switch shows the theme. */
export const fonts = ['1em "DM Sans"', '1em "Playwrite US Trad"'];
