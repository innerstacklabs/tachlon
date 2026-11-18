import { type ChromeContext, type Piece, refocus } from "./parts.js";

/** The keys a scheduler uses most, as docs/KEYS.md names them (a test holds the two together). */
const KEYS: readonly (readonly [string, string])[] = [
  ["Ctrl+F", "Go to the find box"],
  ["Shift+Enter", "Edit the selected task in a dialog"],
  ["c", "Jump to the next critical task"],
  ["Shift+C", "Critical only on or off"],
  ["Shift+L", "Longest path on or off"],
  ["1 to 9", "Collapse every summary to that level; 0 expands all"],
  ["Alt+Left", "Collapse the selected summary"],
  ["Alt+Right", "Expand the selected summary"],
  ["*", "Expand the selected summary and every summary below it"],
  ["Alt+Shift+Up", "Move the selected row up among its siblings"],
  ["Alt+Shift+Down", "Move the selected row down among its siblings"],
  ["Insert", "Insert a row below the selected row"],
  ["Delete", "Delete the selected link, else the selected row"],
  ["Ctrl+Z", "Undo an edit"],
  ["Ctrl+Y", "Redo an edit"],
  ["s", "Show or hide the status line"],
  ["Escape", "Clear the selection and the cell cursor"],
];

/** The keys list (gs#703): `?` (the chart's `toggle-keys` event) and the toolbar's `?` button open it, Escape or Close shuts it, and the chart gets focus back. */
export function installKeysHelp(chrome: ChromeContext, keys: Piece, dialogs: Piece): void {
  const { root } = chrome;
  const dialog = dialogs.part<HTMLDialogElement>("keys-help");
  const body = dialog.querySelector("tbody")!;
  for (const [key, action] of KEYS) {
    const row = body.insertRow();
    row.insertCell().textContent = key;
    row.insertCell().textContent = action;
  }
  const open = (): void => {
    if (!dialog.open) dialog.showModal();
  };
  const close = (): void => {
    dialog.close();
    refocus(root);
  };
  // Escape closes a modal dialog by itself; every way of closing it ends here.
  dialog.addEventListener("close", () => refocus(root));
  keys.part("keys-button").addEventListener("click", open);
  dialogs.part("keys-help-close").addEventListener("click", close);
  chrome.hear("toggle-keys", () => (dialog.open ? close() : open()));
}
