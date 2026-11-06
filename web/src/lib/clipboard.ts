// Copying to the clipboard, the same way for every copy button.

/**
 * Copies `text`. Rejects, rather than throwing, where the browser offers no clipboard (a page served over plain
 * http) or refuses the write, so every caller reports the failure the same way.
 */
export async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}
