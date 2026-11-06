// Stone (@astryxdesign/theme-stone), loaded only when chosen or previewed (themes/index.tsx).
import "@astryxdesign/theme-stone/theme.css";
import "./stone.css";

export { stoneTheme as theme } from "@astryxdesign/theme-stone/built";
export { default } from "./sheet";

/** Its faces (stone.css), loaded before a switch shows the theme. */
export const fonts = ['1em "Montserrat"'];
