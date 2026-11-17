import type { Ctx, Scene } from "./ctx.mjs";

export const setup: (ctx: Ctx) => Promise<void>;
export const steps: Scene[];
