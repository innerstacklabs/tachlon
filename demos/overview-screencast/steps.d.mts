import type { Ctx } from "../screencast/ctx.mjs";
import type { Step } from "../screencast/steps.mjs";

export const LEAD: number;
export const TAIL: number;
export const steps: Step[];
export function setup(ctx: Ctx): Promise<void>;
