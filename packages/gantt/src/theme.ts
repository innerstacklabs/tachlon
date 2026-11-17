import type { PublicApp } from "./api.js";

let colorContext: CanvasRenderingContext2D | null = null;

function context(): CanvasRenderingContext2D {
  if (colorContext !== null) return colorContext;
  const canvas = document.createElement("canvas");
  canvas.width = 1;
  canvas.height = 1;
  const next = canvas.getContext("2d");
  if (next === null) throw new Error("could not create a canvas context for theme colours");
  colorContext = next;
  return next;
}

function hexByte(value: number): string {
  return Math.round(value).toString(16).padStart(2, "0");
}

function normalizeColor(name: string, value: string): string {
  const ctx = context();
  let normalized: string | null = null;
  for (const sentinel of ["#010203", "#040506"]) {
    ctx.fillStyle = sentinel;
    ctx.fillStyle = value;
    if (ctx.fillStyle !== sentinel) {
      normalized = ctx.fillStyle;
      break;
    }
  }
  if (normalized === null) throw new Error(`${name}: ${JSON.stringify(value)} is not a colour`);

  const shortHex = /^#([0-9a-f])([0-9a-f])([0-9a-f])$/i.exec(normalized);
  if (shortHex !== null) {
    return `#${shortHex[1]}${shortHex[1]}${shortHex[2]}${shortHex[2]}${shortHex[3]}${shortHex[3]}`.toLowerCase();
  }
  if (/^#[0-9a-f]{6}(?:[0-9a-f]{2})?$/i.test(normalized)) return normalized.toLowerCase();

  const rgba = /^rgba\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d*\.?\d+)\s*\)$/i.exec(normalized);
  if (rgba !== null) {
    const red = Number(rgba[1]);
    const green = Number(rgba[2]);
    const blue = Number(rgba[3]);
    const alpha = Number(rgba[4]);
    return `#${hexByte(red)}${hexByte(green)}${hexByte(blue)}${hexByte(alpha * 255)}`;
  }

  throw new Error(`${name}: ${JSON.stringify(value)} is not a colour`);
}

export function readTheme(root: HTMLElement, tokens: string[]): Record<string, string> {
  const style = getComputedStyle(root);
  const theme: Record<string, string> = {};
  for (const token of tokens) {
    const name = `--gantt-${token}`;
    theme[token] = normalizeColor(name, style.getPropertyValue(name).trim());
  }
  return theme;
}

/**
 * The bar look from `--gantt-bar-r` (a length in px) and `--gantt-bar-h` (the bar's share of the row: `62%` or `0.62`).
 * A value that is not one of those throws, like a bad colour does, so a typo is loud.
 */
export function readBarLook(root: HTMLElement): { radius: number; height: number } {
  const style = getComputedStyle(root);
  const radius = /^(\d*\.?\d+)(?:px)?$/.exec(style.getPropertyValue("--gantt-bar-r").trim());
  const height = /^(\d*\.?\d+)(%?)$/.exec(style.getPropertyValue("--gantt-bar-h").trim());
  if (radius === null) throw new Error("--gantt-bar-r is not a length in px");
  if (height === null) throw new Error("--gantt-bar-h is not a percentage or a fraction");
  return { radius: Number(radius[1]), height: height[2] === "%" ? Number(height[1]) / 100 : Number(height[1]) };
}

/**
 * The link look: the lengths in px of `--gantt-terminal-size`, `--gantt-terminal-gap`, `--gantt-preview-w` and
 * `--gantt-selected-w`. A value that is not a length in px throws, like a bad colour does.
 */
export function readLinkLook(root: HTMLElement): [number, number, number, number] {
  const style = getComputedStyle(root);
  const px = (token: string): number => {
    const length = /^(\d*\.?\d+)(?:px)?$/.exec(style.getPropertyValue(`--gantt-${token}`).trim());
    if (length === null) throw new Error(`--gantt-${token} is not a length in px`);
    return Number(length[1]);
  };
  return [px("terminal-size"), px("terminal-gap"), px("preview-w"), px("selected-w")];
}

/**
 * The shape look (tl#1215): the lengths in px of `--gantt-today-dash`, `--gantt-today-gap` and `--gantt-crit-edge-w`, the data-date
 * line's dash and gap and the critical bar's edge, so the two read apart without colour. A value that is not a length in px throws.
 */
export function readShapeLook(root: HTMLElement): [number, number, number] {
  const style = getComputedStyle(root);
  const px = (token: string): number => {
    const length = /^(\d*\.?\d+)(?:px)?$/.exec(style.getPropertyValue(`--gantt-${token}`).trim());
    if (length === null) throw new Error(`--gantt-${token} is not a length in px`);
    return Number(length[1]);
  };
  return [px("today-dash"), px("today-gap"), px("crit-edge-w")];
}

/**
 * Call `apply` (on the next frame, once for any number of changes) when the page's colour scheme changes or the root's `style`, `class` or
 * `data-theme` does. Returns the function that stops watching and cancels a call that is waiting; `mount` adds it to the shell's disposers.
 */
export function watchTheme(root: HTMLElement, apply: () => void): () => void {
  const media = matchMedia("(prefers-color-scheme: dark)");
  let frame: number | null = null;
  const schedule = (): void => {
    if (frame !== null) return;
    frame = requestAnimationFrame(() => {
      frame = null;
      apply();
    });
  };
  media.addEventListener("change", schedule);
  const observer = new MutationObserver(schedule);
  observer.observe(root, { attributes: true, attributeFilter: ["style", "class", "data-theme"] });
  return () => {
    media.removeEventListener("change", schedule);
    observer.disconnect();
    if (frame !== null) cancelAnimationFrame(frame);
  };
}

/**
 * Hand the app the theme the root's CSS gives (its `--gantt-*` tokens, the bar, link and shape looks), now and on each change
 * `watchTheme` sees. Returns the call that hands it over again (the test hook's `syncTheme`) and the call that stops watching.
 */
export function installTheme(root: HTMLElement, app: PublicApp, tokens: string[]): { sync: () => void; stop: () => void } {
  const sync = (): void => {
    try {
      app.set_theme(JSON.stringify(readTheme(root, tokens)));
      const look = readBarLook(root);
      app.set_bar_look(look.radius, look.height);
      app.set_link_look(...readLinkLook(root));
      app.set_shape_look(...readShapeLook(root));
    } catch (err) {
      console.error(err instanceof Error ? err.message : String(err));
    }
  };
  sync();
  return { sync, stop: watchTheme(root, sync) };
}
