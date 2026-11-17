// Types for ctx.mjs, so apps/hosted/tests/journey.spec.ts passes `pnpm typecheck` (apps/hosted/tsconfig.json is strict and does not read .mjs).
import type { Page } from "@playwright/test";

export type Mode = "record" | "test";

export interface Ctx {
  page: Page;
  mode: Mode;
  fps: number;
  box: { x: number; y: number; width: number; height: number };
  ROW: number;
  AXIS: number;
  status(): Promise<any>;
  frames(count?: number): Promise<void>;
  until(source: string | ((arg: any) => unknown), arg?: unknown): Promise<unknown>;
  log(message: string): void;
  setCaption(text: string): Promise<void>;
  extendCaption(ending: string): Promise<void>;
  ring(selector: string, options?: object): Promise<void>;
  canvasBox(): Promise<{ x: number; y: number; width: number; height: number }>;
  hold(seconds: number): Promise<void>;
}

export interface Scene {
  name: string;
  /** The book chapter that owns the scene ("J2"). */
  journey?: string;
  /** True when the scene is cut into a per-scene clip. */
  clip?: boolean;
  /** An expression run in the page before the scene; false fails the scene. */
  pre?: string;
  caption: string | ((ctx: Ctx, result: any) => string | Promise<string>);
  captionEnd?: (ctx: Ctx, result: any) => string | null | Promise<string | null>;
  run(ctx: Ctx): Promise<any>;
}

export const MODES: readonly Mode[];
export function createContext(page: Page, options?: { mode?: Mode; fps?: number }): Promise<Ctx>;
export function runScene(ctx: Ctx, scene: Scene): Promise<unknown>;
export interface SceneWindow {
  name: string;
  clip: boolean;
  /** When the scene started and ended, in ms on the `now` clock. */
  startMs: number;
  endMs: number;
}
export function runScenes(ctx: Ctx, list: Scene[], options?: { now?: () => number }): Promise<{ run: string[]; windows: SceneWindow[] }>;
