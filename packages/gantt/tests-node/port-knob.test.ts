import { test } from "node:test";
import { expect } from "@playwright/test";
import { previewPort } from "../tests/support/knobs.ts";

// gs#378: a local run must not meet the vite preview of the CI runner (4173) or of another checkout.
test("a local run does not default to the port the CI runner serves on", () => {
  const local = previewPort({}, "/work/checkouts/gantt-378");
  expect(local, "local default is not 4173").not.toBe(4173);
  expect(local).toBeGreaterThanOrEqual(4400);
  expect(local).toBeLessThan(4900);
});

test("two checkouts get their own default ports, the same checkout the same one", () => {
  const roots = Array.from({ length: 12 }, (_, i) => `/work/checkouts/gantt-${i}`);
  const ports = roots.map((root) => previewPort({}, root));
  expect(new Set(ports).size, `ports for ${roots.length} checkouts: ${ports.join(" ")}`).toBeGreaterThan(8);
  expect(previewPort({}, roots[0])).toBe(ports[0]);
});

test("GANTT_PORT overrides, and CI without it keeps 4173", () => {
  expect(previewPort({ GANTT_PORT: "4391" }, "/a")).toBe(4391);
  expect(previewPort({ GANTT_PORT: "4174", CI: "true" }, "/a")).toBe(4174);
  expect(previewPort({ CI: "true" }, "/a")).toBe(4173);
});
