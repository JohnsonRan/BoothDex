// Chocolate (@astryxdesign/theme-chocolate), loaded only when chosen or previewed (themes/index.tsx).
import "@astryxdesign/theme-chocolate/theme.css";
import "./chocolate.css";

export { chocolateTheme as theme } from "@astryxdesign/theme-chocolate/built";
export { default } from "./sheet";

/** Its faces (chocolate.css), loaded before a switch shows the theme. */
export const fonts = ['1em "Albert Sans"', '1em "Fraunces"'];
