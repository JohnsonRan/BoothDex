/** A theme module's default export: nothing to draw. The root renders it through next/dynamic (themes/index.tsx),
 *  which is what makes the server send the module's stylesheet with the page. */
export default function ThemeModuleSheet() {
  return null;
}
