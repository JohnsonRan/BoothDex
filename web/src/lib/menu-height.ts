// DropdownMenu caps its height at 300 px (DropdownMenu.js MENU_BLOCK_CAP) while its rows are in rem, which grow
// with the page's 115 % scale above 768 px and again with a larger browser default font: at Chrome's "Very large"
// (24 px) the five-row sort menu passes 300 px and scrolls. `menuMaxHeight` lifts that cap and the viewport still
// bounds the menu (DropdownMenu.d.ts:141-146), so a menu of a fixed handful of rows shows whole. The currency
// menu keeps the default: a long list may scroll, and 300 px still shows five of its twelve rows at a 20 px font.

/** The cap of the site's DropdownMenus of a fixed handful of rows, in px (the prop takes a number of pixels). */
export const MENU_MAX_HEIGHT = 480;
