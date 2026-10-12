// Native modal dialogs: `showModal()` makes the rest of the page inert and
// gives the dialog a top layer. This action adds what the platform leaves out:
// a Tab sequence that wraps inside the dialog, initial focus, Escape and
// backdrop clicks routed to `onclose`, and focus restoration on close.
//
// Usage: `<dialog class="modal" use:modal={close} aria-labelledby="x-title">`.
// Mark the control that should receive focus first with `data-initial-focus`;
// otherwise the first control in the dialog does.

const FOCUSABLE = 'button:not(:disabled), input:not(:disabled):not([type="hidden"]), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])';

/** The last element the user focused outside of dialogs and menus: where focus returns when a dialog opened from a menu closes. */
let lastTrigger: HTMLElement | null = null;
if (typeof document !== "undefined") {
  document.addEventListener("focusin", (e) => {
    const target = e.target;
    if (target instanceof HTMLElement && !target.closest("dialog, .menu, .palette")) lastTrigger = target;
  }, true);
}

const controls = (node: HTMLElement) => Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => el.getClientRects().length);

/** Move focus to the dialog's preferred first control. */
export function focusInitial(node: HTMLElement) {
  const target = node.querySelector<HTMLElement>("[data-initial-focus]:not(:disabled)") ?? controls(node)[0] ?? node;
  target.focus({ preventScroll: true });
  return target;
}

export function modal(node: HTMLDialogElement, onclose?: () => void) {
  let close = onclose;
  const active = document.activeElement;
  const previous = active instanceof HTMLElement && active !== document.body ? active : lastTrigger;
  let pressed: EventTarget | null = null;

  function trap(event: KeyboardEvent) {
    if (event.key !== "Tab") return;
    const list = controls(node);
    const first = list[0], last = list.at(-1);
    if (!first) { event.preventDefault(); return; }
    const current = document.activeElement;
    if (event.shiftKey && (current === first || current === node)) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && (current === last || current === node)) { event.preventDefault(); first.focus(); }
    else if (!node.contains(current)) { event.preventDefault(); first.focus(); }
  }
  const cancel = (event: Event) => { event.preventDefault(); close?.(); };
  const down = (event: MouseEvent) => { pressed = event.target; };
  function click(event: MouseEvent) {
    // Only a press that started on the backdrop counts, so dragging a text selection out of a field doesn't close it.
    if (event.target !== node || pressed !== node) return;
    const r = node.getBoundingClientRect();
    if (event.clientX < r.left || event.clientX > r.right || event.clientY < r.top || event.clientY > r.bottom) close?.();
  }
  // Content that loads after opening replaces the focused control, or brings the control that should have had
  // focus all along. Keep focus inside, and move it only if the user hasn't moved it yet.
  let auto: Element | null = null;
  const observer = new MutationObserver(() => {
    const current = document.activeElement;
    if (!current || current === document.body || current === node || !node.contains(current)) auto = focusInitial(node);
    else if (current === auto) {
      const wanted = node.querySelector<HTMLElement>("[data-initial-focus]:not(:disabled)");
      if (wanted && wanted !== current) { wanted.focus({ preventScroll: true }); auto = wanted; }
    }
  });

  node.addEventListener("keydown", trap);
  node.addEventListener("cancel", cancel);
  node.addEventListener("mousedown", down);
  node.addEventListener("click", click);
  if (!node.open) node.showModal();
  auto = focusInitial(node);
  observer.observe(node, { childList: true, subtree: true });

  return {
    update(next?: () => void) { close = next; },
    destroy() {
      observer.disconnect();
      node.removeEventListener("keydown", trap);
      node.removeEventListener("cancel", cancel);
      node.removeEventListener("mousedown", down);
      node.removeEventListener("click", click);
      if (node.open) node.close();
      const back = previous?.isConnected ? previous : lastTrigger?.isConnected ? lastTrigger : null;
      back?.focus({ preventScroll: true });
    },
  };
}
