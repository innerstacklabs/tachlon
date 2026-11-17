import { optional } from "./dom.js";
import { type Shell } from "./shell.js";

/** The Fit button (gs#703): one press shows the whole project, the way back after a zoom or a scroll away. */
export function installFitButton({ root, app }: Shell): void {
  optional<HTMLButtonElement>(root, "fit-button")?.addEventListener("click", () => {
    app.fit_project();
    root.focus({ preventScroll: true });
  });
}
