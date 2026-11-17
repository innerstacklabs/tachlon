import { type Shell } from "./shell.js";

/** The print legend's entries (crates/app/src/print/legend.rs): the label and the theme token its swatch takes. tests/legend.spec.ts keeps the two lists equal. */
const ENTRIES: readonly (readonly [label: string, token: string])[] = [
  ["Task", "bar"],
  ["Critical", "crit"],
  ["Deadline", "crit"],
  ["Start constraint", "text"],
  ["Finish constraint", "text"],
  ["Summary", "sum"],
  ["Dependency", "link"],
  ["Data date", "today"],
  ["Non-working day", "nonwork"],
  ["Proposal ghost", "sel-edge"],
  ["Just-moved outline", "sel-edge"],
];

/** The legend strip under the canvas (gs#703): one swatch and one label per print legend entry. The swatch colour is the theme token's CSS custom property, so it follows the theme and a host's override with no code here. Built once at mount, never per frame. */
export function installLegend({ root }: Shell): void {
  const list = root.querySelector<HTMLUListElement>("[data-part=legend]");
  if (list === null) return;
  // Other installs append their own dialogs to the root (export.ts); the strip is the last row of the grid, so it moves to the end.
  root.append(list);
  list.replaceChildren(
    ...ENTRIES.map(([label, token]) => {
      const item = document.createElement("li");
      const swatch = document.createElement("span");
      swatch.className = "legend-swatch";
      swatch.style.setProperty("--legend-swatch", `var(--gantt-${token})`);
      swatch.setAttribute("aria-hidden", "true");
      item.append(swatch, label);
      return item;
    }),
  );
}
