// The specs read the chart through `window.gantt`, the chart's internal test hook (gantt#1090). The built package's types leave an
// internal hook out, so the hosted project declares it here, from the one file that defines it.
export {};

declare global {
  interface Window {
    gantt?: import("../../../packages/gantt/src/test-hook.ts").GanttHook;
  }
}
