// Gothic (@astryxdesign/theme-gothic), loaded only when chosen or previewed (themes/index.tsx).
import "@astryxdesign/theme-gothic/theme.css";
import "./gothic.css";

export { gothicTheme as theme } from "@astryxdesign/theme-gothic/built";
export { default } from "./sheet";

/** Its faces (gothic.css), loaded before a switch shows the theme. */
export const fonts = ['1em "Fustat"', '400 1em "Manufacturing Consent"'];
