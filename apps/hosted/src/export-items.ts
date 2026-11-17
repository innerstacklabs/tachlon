// The file menu's two list exports (gs#703, gantt#1875), under Save: "Export the list as CSV" and "Export the list as Excel". Each is a div menuitem, not a button,
// so the sample list's buttons stay the samples and Save; the page's `<template>` holds the markup and this file gives it its click and key handlers.
import type { Gantt } from "gantt-core";

/** Place one export item from its `<template>`; choosing it (a click, Enter or Space) closes the menu and runs `run`. */
function placeExport(list: HTMLElement, button: () => HTMLElement, template: string, mark: string, run: () => void | Promise<void>): void {
  const found = document.getElementById(template);
  if (found === null) throw new Error(`the host page has no #${template}`);
  const items = (found as HTMLTemplateElement).content.cloneNode(true) as DocumentFragment;
  const item = items.querySelector<HTMLElement>(`[${mark}]`)!;
  const choose = (): void => {
    list.hidden = true;
    button().setAttribute("aria-expanded", "false");
    void Promise.resolve(run()).catch((err: unknown) => console.error(`hosted: ${mark} failed`, err));
  };
  item.addEventListener("click", choose);
  item.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    choose();
  });
  list.append(items);
}

/** "Export the list as CSV" goes under Save; it downloads the visible rows as `<name>.csv`, named after the opened file (`untitled` for a sample). */
export function placeCsvExport(list: HTMLElement, button: () => HTMLElement, gantt: Gantt): void {
  placeExport(list, button, "export-csv-item", "data-export-csv", () => gantt.exportCsv());
}

/** "Export the list as Excel" goes under "Export the list as CSV"; it downloads the same rows as `<name>.xlsx`. */
export function placeExcelExport(list: HTMLElement, button: () => HTMLElement, gantt: Gantt): void {
  placeExport(list, button, "export-xlsx-item", "data-export-xlsx", () => gantt.exportXlsx());
}
