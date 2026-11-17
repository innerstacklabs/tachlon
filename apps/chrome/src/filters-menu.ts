import type { PlainFilter, ViewState } from "gantt-core";
import { installCheckFilter } from "./check-filter.js";
import { installCodeFilter } from "./code-filter.js";
import { installGroupBy } from "./group-by.js";
import { installLookahead } from "./lookahead.js";
import type { ChromeContext, Piece } from "./parts.js";
import { installResourceFilter } from "./resource-filter.js";
import { installStatusFilter } from "./status-filter.js";

export interface FiltersMenu {
  /** The button's count and each control's pressed state, from the view state; the DOM is written only when it changes. */
  sync: (state: ViewState) => void;
  /** A file was opened in place: show its activity codes and the chart's lookahead window. */
  reset: () => void;
}

/** The controls the arrows walk: whatever in the panel is shown and enabled. The Status, Check and Resource choices are left out: their own arrows pick an entry (Tab reaches it). */
const items = (panel: HTMLElement): HTMLElement[] =>
  [...panel.querySelectorAll<HTMLElement>("button, select:not([data-part=status-filter]):not([data-part=check-filter]):not([data-part=resource-filter]):not([data-part=group-by]), input")].filter((el) => !el.hidden && el.offsetParent !== null && !(el as HTMLButtonElement).disabled);

/**
 * The Filters menu (gs#808): one button in the toolbar that opens a panel holding the filters (Critical only, Longest path, Near critical,
 * Slipped, Joined critical, Left critical, Float lost, Show moved, the activity code choice, Status, Check, Resource, Start constraint and Lookahead), so the strip stays one
 * line however many are on. It drives the chart only through `commands.setFilter` and `clearFilters`, and shows what `state()` and the
 * `change` event say: the button reads "Filters · 2" with two on, "Filters" with none. Enter or Space on the button opens it and puts
 * focus on the first item; the arrows walk the buttons, Escape closes it and puts focus back on the button; a press outside it, or focus
 * leaving it, closes it. Show moved and the baseline's filters are read when the panel opens and after a press or a choice inside it.
 */
export function installFiltersMenu(chrome: ChromeContext, menu: Piece): FiltersMenu {
  const { gantt, root } = chrome;
  const button = menu.part<HTMLButtonElement>("filters-button");
  const panel = menu.part<HTMLElement>("filters-panel");
  const statusFilter = installStatusFilter(chrome, menu);
  const checkFilter = installCheckFilter(chrome, menu);
  const codeFilter = installCodeFilter(chrome, menu);
  const resourceFilter = installResourceFilter(chrome, menu);
  const groupBy = installGroupBy(chrome, menu);
  const lookahead = installLookahead(chrome, menu);
  const clear = menu.part<HTMLButtonElement>("filters-clear");
  const moved = menu.part<HTMLButtonElement>("moved-filter");
  const floatLost = menu.part<HTMLButtonElement>("float-lost-toggle");
  const floatTitle = floatLost.title;
  const movedTitle = moved.title;
  const slipped = menu.part<HTMLButtonElement>("slipped-toggle");
  // Each row filter's button, by the filter's public name.
  const toggles: [HTMLButtonElement, Exclude<PlainFilter, "off">][] = [
    [menu.part("critical-toggle"), "critical"],
    [menu.part("longest-toggle"), "longestPath"],
    [menu.part("near-critical-toggle"), "nearCritical"],
    [slipped, "slipped"],
    [menu.part("joined-critical-toggle"), "joinedCritical"],
    [menu.part("left-critical-toggle"), "leftCritical"],
    [floatLost, "floatLost"],
    [moved, "moved"],
    [menu.part("start-constraint-toggle"), "startConstraint"],
  ];
  const close = (refocus: boolean): void => {
    if (panel.hidden) return;
    panel.hidden = true;
    root.removeEventListener("keydown", afterKey);
    button.setAttribute("aria-expanded", "false");
    if (refocus) button.focus({ preventScroll: true });
  };
  /** The items that read the baseline and the move report: shown with a baseline, enabled when there is something to keep. */
  const showMoved = (): void => {
    const state = gantt.state();
    const none = state.baseline === null;
    // A baseline opened from a file has dates and no float, so there is nothing to compare.
    const hasFloat = !none && gantt.filterInputs().baselineHasFloat;
    floatLost.hidden = none;
    floatLost.disabled = !hasFloat;
    floatLost.title = hasFloat || none ? floatTitle : "This baseline has no float to compare";
    // Joined and Left critical read the baseline, so there is nothing to press without one.
    toggles[4]![0].hidden = none;
    toggles[5]![0].hidden = none;
    moved.disabled = state.moved === null && state.filter !== "moved";
    // The key does nothing while the button is disabled, so the title says why instead of naming it.
    moved.title = moved.disabled ? "Nothing has moved yet: drag a bar or change a date, then this shows what moved." : movedTitle;
  };
  // A key can change the filter without a click or a choice (Shift+C, Shift+L reach the chart's keys), so while the panel is open each
  // key on the chart's root refreshes Show moved, a tick later so the chart's own handler has run. Nothing listens while it is shut.
  let pending: ReturnType<typeof setTimeout> | undefined;
  const afterKey = (): void => {
    pending = setTimeout(showMoved, 0);
  };
  const open = (): void => {
    showMoved();
    root.addEventListener("keydown", afterKey);
    panel.hidden = false;
    button.setAttribute("aria-expanded", "true");
    items(panel)[0]?.focus({ preventScroll: true });
  };
  button.addEventListener("click", () => (panel.hidden ? open() : close(true)));
  const within = (node: Node): boolean => button.contains(node) || panel.contains(node);
  // The keys are the menu's, not the chart's: Escape here must not also clear the chart's selection. It is taken on the way down,
  // before the activity code choices' own keydown handlers (which stop every key) can hide it.
  const escape = (e: KeyboardEvent): void => {
    if (e.key !== "Escape" || panel.hidden) return;
    e.preventDefault();
    e.stopPropagation();
    close(true);
  };
  const keys = (e: KeyboardEvent): void => {
    // Enter and Space on the button are the button's: bubbling them would let the chart's cursor open the selected cell's editor.
    if (e.target === button && (e.key === "Enter" || e.key === " ")) e.stopPropagation();
    if (panel.hidden || !(e.target instanceof HTMLButtonElement) || e.target === button) return;
    const list = items(panel);
    const at = list.indexOf(e.target);
    const next = { ArrowDown: at + 1, ArrowRight: at + 1, ArrowUp: at - 1, ArrowLeft: at - 1, Home: 0, End: list.length - 1 }[e.key];
    if (next === undefined) return;
    e.preventDefault();
    e.stopPropagation();
    list[(next + list.length) % list.length]?.focus({ preventScroll: true });
  };
  for (const el of [button, panel]) {
    el.addEventListener("keydown", escape, true);
    el.addEventListener("keydown", keys);
    el.addEventListener("focusout", (e) => {
      if (e.relatedTarget instanceof Node && !within(e.relatedTarget)) close(false);
    });
  }
  const press = (e: PointerEvent): void => {
    if (!(e.target instanceof Node) || !within(e.target)) close(false);
  };
  // The listeners are on the host's element: they go when the chrome does, open or not.
  root.addEventListener("pointerdown", press);
  chrome.owns(() => {
    root.removeEventListener("pointerdown", press);
    root.removeEventListener("keydown", afterKey);
    clearTimeout(pending);
  });
  // Find searches again on the `change` each of these makes, so the handlers only set the filter.
  for (const [el, name] of toggles) {
    el.addEventListener("click", () => {
      gantt.commands.setFilter(gantt.state().filter === name ? "off" : name);
    });
  }
  // Clear filters turns the row filter and Lookahead off at once; the button hides itself (in sync) once nothing is on.
  clear.addEventListener("click", () => {
    gantt.commands.clearFilters();
  });
  // A press or a choice in the panel can turn another filter on or off, which changes whether Show moved is pressed.
  panel.addEventListener("click", showMoved);
  panel.addEventListener("change", showMoved);
  let count = -1;
  let baseline: boolean | null = null;
  const sync = (state: ViewState): void => {
    for (const [el, name] of toggles) {
      const pressed = String(state.filter === name);
      if (el.getAttribute("aria-pressed") !== pressed) el.setAttribute("aria-pressed", pressed);
    }
    // Slipped reads the baseline, so there is nothing to press without one.
    if ((state.baseline !== null) !== baseline) {
      baseline = state.baseline !== null;
      slipped.hidden = !baseline;
    }
    statusFilter.sync(state);
    checkFilter.sync(state);
    codeFilter.sync(state);
    resourceFilter.sync(state);
    groupBy.sync(state);
    lookahead.sync(state);
    const on = (state.filter === "off" ? 0 : 1) + (state.lookahead.on ? 1 : 0);
    if (on === count) return;
    count = on;
    button.textContent = on === 0 ? "Filters" : `Filters · ${on}`;
    button.dataset.on = String(on > 0);
    // Pressed with the arrows or Enter, it vanishes: put focus on the first item, not on the page.
    if (on === 0 && clear === document.activeElement) items(panel)[0]?.focus({ preventScroll: true });
    clear.hidden = on === 0;
  };
  sync(gantt.state());
  return {
    sync,
    reset: () => {
      codeFilter.reset();
      resourceFilter.reset();
      groupBy.reset();
      lookahead.reset();
    },
  };
}
