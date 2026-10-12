// Keyboard behavior for popup menus (`.menu`): focus moves into the menu when it opens,
// arrows/Home/End move between items, Tab and Escape close it, and focus goes back to where it was.

const ITEMS = "button:not(:disabled)";

export function menu(node: HTMLElement, onclose?: () => void) {
  let close = onclose;
  const active = document.activeElement;
  const previous = active instanceof HTMLElement && active !== document.body ? active : null;
  for (const b of node.querySelectorAll("button")) b.setAttribute("role", "menuitem");
  node.setAttribute("role", "menu");
  const items = () => Array.from(node.querySelectorAll<HTMLElement>(ITEMS));

  function key(e: KeyboardEvent) {
    const list = items();
    const i = list.indexOf(document.activeElement as HTMLElement);
    let next: number | null = null;
    if (e.key === "ArrowDown") next = (i + 1) % list.length;
    else if (e.key === "ArrowUp") next = (i <= 0 ? list.length : i) - 1;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = list.length - 1;
    else if (e.key === "Tab") { close?.(); return; }
    else if (e.key === "Escape") { e.stopPropagation(); close?.(); return; }
    if (next === null || !list.length) return;
    e.preventDefault();
    list[next].focus();
  }
  node.addEventListener("keydown", key);
  items()[0]?.focus({ preventScroll: true });

  return {
    update(next?: () => void) { close = next; },
    destroy() {
      node.removeEventListener("keydown", key);
      const current = document.activeElement;
      // Only take focus back if it was left in the menu (a dialog opened by an item keeps it).
      if ((!current || current === document.body || node.contains(current)) && previous?.isConnected) previous.focus({ preventScroll: true });
    },
  };
}
