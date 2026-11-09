// The site's dialogs (settings, item detail) take the viewport's height less 1 rem above and below, up to 50 rem
// (settings-dialog template's size). The item dialog used Astryx Dialog's default 75dvh, which on a short window
// left it a quarter shorter than settings for no reason (owner, 2026-11-09).

/** The height cap of the site's large dialogs. */
export const DIALOG_MAX_HEIGHT = "min(50rem, calc(100dvh - 2rem))";
