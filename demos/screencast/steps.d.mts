import type { Ctx, Scene } from "./ctx.mjs";

/** A screencast step: a scene that may declare its spoken line. */
export interface Step extends Scene {
  /** The line spoken over the step; the recorder holds the step until its audio (`audioSeconds`) has played. */
  say?: string;
}

export const setup: (ctx: Ctx) => Promise<void>;
export const steps: Step[];
