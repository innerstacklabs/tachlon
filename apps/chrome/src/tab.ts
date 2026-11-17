import { PRODUCT_NAME, type ViewState } from "gantt-core";
import { type ChromeContext } from "./parts.js";

/** What the browser tab shows for a chart with a file open: the name, behind a bullet while there are unsaved changes. */
export function tabTitle(name: string, unsaved: boolean): string {
  return `${unsaved ? "• " : ""}${name} - ${PRODUCT_NAME}`;
}

/**
 * The tab names the open file and marks unsaved changes (gs#703), and leaving with unsaved changes asks first. Both read
 * `state().fileName` and `unsaved`. A chart with no file open leaves the page title alone until the tab has named a file; a schedule
 * loaded with no file name after that (New schedule) reads `untitled`, behind a bullet once it is edited, so the tab never keeps the last file's name.
 * Taking the chrome off puts back the title the page had and removes the question.
 */
export function installTab(chrome: ChromeContext): (state: ViewState) => void {
  const title = document.title;
  let written = false;
  let unsaved = false;
  const ask = (event: BeforeUnloadEvent): void => {
    if (!unsaved) return;
    // The browser shows its own box; the text it shows is not ours to set.
    event.preventDefault();
    event.returnValue = "";
  };
  window.addEventListener("beforeunload", ask);
  chrome.owns(() => {
    window.removeEventListener("beforeunload", ask);
    if (written) document.title = title;
  });
  return (state) => {
    unsaved = state.unsaved;
    if (state.fileName === null && !written) return;
    written = true;
    const next = state.fileName === null ? tabTitle("untitled", unsaved) : tabTitle(state.fileName, unsaved);
    if (document.title !== next) document.title = next;
  };
}
