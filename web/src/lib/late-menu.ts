// Menus whose code loads after the page (next/dynamic, ssr: false: the language, currency, sort and account
// menus) draw the button they open from until then. That button has no menu yet, and a press on it comes after
// hydration, so lib/early-clicks.ts leaves it alone. The press is kept here instead, and the menu opens from its
// own trigger once its code is there: by keyboard as by keyboard, by pointer as by pointer. A press elsewhere
// meanwhile, or one older than 10 s, drops it.
import { useEffect } from "react";

type LateMenu = "language" | "currency" | "sort" | "account";

const KEEP_MS = 10_000;
// the menus pressed before their code came: when, and the click's `detail` (0 by keyboard)
const pressed = new Map<LateMenu, { at: number; detail: number }>();

const mark = (menu: LateMenu) => ({ "data-late-menu": menu }) as const;

/** The waiting button's props: the menu's mark, and a click kept for the menu. */
export const lateMenuButton = (menu: LateMenu) => ({
  ...mark(menu),
  onClick: (e: React.MouseEvent) => {
    pressed.set(menu, { at: Date.now(), detail: e.detail });
    addEventListener(
      "pointerdown",
      (down) => {
        const on =
          down.target instanceof Element &&
          down.target.closest(`[data-late-menu="${menu}"]`);
        if (!on) pressed.delete(menu);
      },
      { capture: true, once: true },
    );
  },
});

/** In the loaded menu: its trigger's mark (spread onto it); opens the menu if its button was pressed. */
export function useLateMenu(menu: LateMenu) {
  useEffect(() => {
    if (!pressed.has(menu)) return;
    // a task later: opened in the mount's own effects, the menu showed without moving focus into it
    const timer = setTimeout(() => {
      const press = pressed.get(menu);
      pressed.delete(menu);
      const trigger = document.querySelector<HTMLElement>(
        `[data-late-menu="${menu}"]`,
      );
      if (!press || !trigger || Date.now() - press.at > KEEP_MS) return;
      // focused first, so closing the menu hands focus back to it
      trigger.focus({ preventScroll: true });
      trigger.dispatchEvent(
        new MouseEvent("click", {
          bubbles: true,
          cancelable: true,
          detail: press.detail,
        }),
      );
    });
    return () => clearTimeout(timer);
  }, [menu]);
  return mark(menu);
}
