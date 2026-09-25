/** Native modal semantics provide inert background and a contained Tab sequence. */
export function modal(node: HTMLDialogElement) {
  const previous = document.activeElement;
  function trap(event: KeyboardEvent) {
    if (event.key !== "Tab") return;
    const controls = Array.from(node.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]')).filter((el) => el.getClientRects().length);
    const first = controls[0], last = controls.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }
  node.addEventListener("keydown", trap);
  node.showModal();
  node.querySelector<HTMLElement>("[data-initial-focus]")?.focus();
  return { destroy() {
    node.removeEventListener("keydown", trap);
    node.close();
    if (previous instanceof HTMLElement && previous.isConnected) previous.focus({ preventScroll: true });
  } };
}
